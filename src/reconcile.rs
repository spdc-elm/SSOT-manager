use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::config::{Config, ConfigDiagnostic, ResolvedProfile, SyncIntent, resolve_profile};
use crate::paths::{path_to_string, resolved_link_target, symlink_target_for};
use crate::state::{
    ApplyJournal, JournalEntry, ManagedState, PathState, StateStore, build_record, create_symlink,
    now_timestamp, remove_existing_path, restore_path, snapshot_path, symlink_matches_expected,
};

#[derive(Debug, Clone)]
pub struct Plan {
    pub profile_name: String,
    pub items: Vec<PlanItem>,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct PlanItem {
    pub action: Action,
    pub target: PathBuf,
    pub desired_source: Option<PathBuf>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Create,
    Update,
    Remove,
    Skip,
    Warning,
    Danger,
}

#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub journal: ApplyJournal,
    pub plan: Plan,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub profile_name: String,
    pub issues: Vec<DoctorIssue>,
}

#[derive(Debug, Clone)]
pub struct DoctorIssue {
    pub kind: DoctorIssueKind,
    pub target: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum DoctorIssueKind {
    BrokenSymlink,
    MissingManagedTarget,
    ManagedDrift,
}

#[derive(Debug, Clone)]
pub struct UndoResult {
    pub profile_name: String,
    pub reverted_targets: Vec<PathBuf>,
}

pub fn build_plan(config: &Config, profile_name: &str, state: &ManagedState) -> Result<Plan> {
    let resolved = resolve_profile(config, profile_name)?;
    plan_from_resolved(resolved, state)
}

pub fn apply_plan(plan: Plan, state: &ManagedState, store: &StateStore) -> Result<ApplyResult> {
    if plan.items.iter().any(|item| item.action == Action::Danger) {
        bail!("refusing to apply because the plan contains danger actions");
    }

    let timestamp = now_timestamp()?;
    let desired_sources = desired_source_map(&plan);
    let mut next_state = state.clone();
    let mut journal_entries = Vec::new();

    for item in &plan.items {
        match item.action {
            Action::Create | Action::Update => {
                let desired_source = item
                    .desired_source
                    .as_ref()
                    .expect("desired source is required for create/update");
                let before = snapshot_path(&item.target)?;
                let record_before = next_state
                    .records
                    .get(&path_to_string(&item.target))
                    .cloned();

                remove_existing_path(&item.target)?;
                let link_target = symlink_target_for(desired_source, &item.target);
                create_symlink(&item.target, &link_target)?;

                let after = snapshot_path(&item.target)?;
                if !symlink_matches_expected(&item.target, &after, desired_source) {
                    bail!(
                        "verification failed for {} after {}",
                        item.target.display(),
                        item.action.as_str()
                    );
                }

                let record_after = Some(build_record(
                    &SyncIntent {
                        profile_name: plan.profile_name.clone(),
                        source: desired_source.clone(),
                        target: item.target.clone(),
                        mode: crate::config::MaterializationMode::Symlink,
                    },
                    timestamp,
                ));

                if let Some(record) = &record_after {
                    next_state
                        .records
                        .insert(path_to_string(&item.target), record.clone());
                }

                journal_entries.push(JournalEntry {
                    action: item.action.as_str().to_string(),
                    target: path_to_string(&item.target),
                    before,
                    after,
                    record_before,
                    record_after,
                });
            }
            Action::Remove => {
                let before = snapshot_path(&item.target)?;
                let record_before = next_state
                    .records
                    .get(&path_to_string(&item.target))
                    .cloned();

                match before {
                    PathState::Missing => {}
                    PathState::Symlink { .. } => remove_existing_path(&item.target)?,
                    PathState::File | PathState::Directory | PathState::Other => bail!(
                        "refusing to remove '{}' because it drifted away from a managed symlink",
                        item.target.display()
                    ),
                }

                let after = snapshot_path(&item.target)?;
                next_state.records.remove(&path_to_string(&item.target));

                journal_entries.push(JournalEntry {
                    action: item.action.as_str().to_string(),
                    target: path_to_string(&item.target),
                    before,
                    after,
                    record_before,
                    record_after: None,
                });
            }
            Action::Skip => {
                if let Some(existing) = next_state.records.get_mut(&path_to_string(&item.target)) {
                    if existing.profile == plan.profile_name {
                        existing.updated_at = timestamp;
                        if let Some(desired_source) = &item.desired_source {
                            existing.source = path_to_string(desired_source);
                        }
                    }
                }
            }
            Action::Warning | Action::Danger => {}
        }
    }

    let journal = ApplyJournal {
        profile: plan.profile_name.clone(),
        applied_at: timestamp,
        entries: journal_entries,
    };

    verify_plan_state(&plan, &desired_sources)?;

    store.save(&next_state)?;
    store.write_last_apply(&journal)?;

    Ok(ApplyResult { journal, plan })
}

pub fn doctor_profile(
    config: &Config,
    profile_name: &str,
    state: &ManagedState,
) -> Result<DoctorReport> {
    // Validate the profile exists even though doctor primarily relies on state.
    let _ = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow::anyhow!("unknown profile '{profile_name}'"))?;

    let mut issues = Vec::new();
    for record in state
        .records
        .values()
        .filter(|record| record.profile == profile_name)
    {
        let target = PathBuf::from(&record.target);
        let current = snapshot_path(&target)?;
        match current {
            PathState::Missing => issues.push(DoctorIssue {
                kind: DoctorIssueKind::MissingManagedTarget,
                target,
                message: "managed target is missing".to_string(),
            }),
            PathState::Symlink {
                target: current_target,
            } => {
                let resolved = resolved_link_target(&target, Path::new(&current_target));
                if !resolved.exists() {
                    issues.push(DoctorIssue {
                        kind: DoctorIssueKind::BrokenSymlink,
                        target,
                        message: format!(
                            "managed symlink points to missing path {}",
                            resolved.display()
                        ),
                    });
                } else if resolved != PathBuf::from(&record.source) {
                    issues.push(DoctorIssue {
                        kind: DoctorIssueKind::ManagedDrift,
                        target,
                        message: format!(
                            "managed symlink points to {} instead of {}",
                            resolved.display(),
                            record.source
                        ),
                    });
                }
            }
            PathState::File | PathState::Directory | PathState::Other => issues.push(DoctorIssue {
                kind: DoctorIssueKind::ManagedDrift,
                target,
                message: "managed target was replaced by non-symlink content".to_string(),
            }),
        }
    }

    issues.sort_by(|left, right| path_to_string(&left.target).cmp(&path_to_string(&right.target)));

    Ok(DoctorReport {
        profile_name: profile_name.to_string(),
        issues,
    })
}

pub fn undo_last_apply(store: &StateStore) -> Result<UndoResult> {
    let journal = store.load_last_apply()?;
    let mut state = store.load()?;

    for entry in &journal.entries {
        let target = PathBuf::from(&entry.target);
        let current = snapshot_path(&target)?;
        if current != entry.after {
            bail!(
                "refusing to undo because {} no longer matches the recorded post-apply state",
                target.display()
            );
        }
    }

    let mut reverted_targets = Vec::new();
    for entry in journal.entries.iter().rev() {
        let target = PathBuf::from(&entry.target);
        restore_path(&target, &entry.before)?;

        match &entry.record_before {
            Some(record) => {
                state.records.insert(entry.target.clone(), record.clone());
            }
            None => {
                state.records.remove(&entry.target);
            }
        }

        reverted_targets.push(target);
    }

    store.save(&state)?;
    store.clear_last_apply()?;

    Ok(UndoResult {
        profile_name: journal.profile,
        reverted_targets,
    })
}

fn plan_from_resolved(resolved: ResolvedProfile, state: &ManagedState) -> Result<Plan> {
    let mut items = Vec::new();
    let mut desired_targets = BTreeSet::new();

    for intent in &resolved.intents {
        let current = snapshot_path(&intent.target)?;
        let target_key = path_to_string(&intent.target);
        let record = state.records.get(&target_key);
        desired_targets.insert(target_key.clone());

        let item = if symlink_matches_expected(&intent.target, &current, &intent.source) {
            PlanItem {
                action: Action::Skip,
                target: intent.target.clone(),
                desired_source: Some(intent.source.clone()),
                reason: "target already matches desired symlink".to_string(),
            }
        } else {
            match current {
                PathState::Missing => PlanItem {
                    action: Action::Create,
                    target: intent.target.clone(),
                    desired_source: Some(intent.source.clone()),
                    reason: "target does not exist".to_string(),
                },
                PathState::Symlink { .. } => match record {
                    Some(record) if record.profile == resolved.profile_name => PlanItem {
                        action: Action::Update,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        reason: "managed symlink points to a different source".to_string(),
                    },
                    Some(_) => PlanItem {
                        action: Action::Danger,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        reason: "target is managed by another profile".to_string(),
                    },
                    None => PlanItem {
                        action: Action::Danger,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        reason: "target is an unmanaged symlink that would be replaced".to_string(),
                    },
                },
                PathState::File | PathState::Directory | PathState::Other => match record {
                    Some(record) if record.profile == resolved.profile_name => PlanItem {
                        action: Action::Warning,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        reason: "managed target drifted into non-symlink content; inspect before mutating"
                            .to_string(),
                    },
                    Some(_) => PlanItem {
                        action: Action::Danger,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        reason: "target is managed by another profile".to_string(),
                    },
                    None => PlanItem {
                        action: Action::Danger,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        reason: "target contains unmanaged content".to_string(),
                    },
                },
            }
        };

        items.push(item);
    }

    for record in state
        .records
        .values()
        .filter(|record| record.profile == resolved.profile_name)
    {
        if desired_targets.contains(&record.target) {
            continue;
        }

        let target = PathBuf::from(&record.target);
        let current = snapshot_path(&target)?;
        let action = match current {
            PathState::Missing | PathState::Symlink { .. } => Action::Remove,
            PathState::File | PathState::Directory | PathState::Other => Action::Warning,
        };
        let reason = match action {
            Action::Remove => {
                "target was previously managed by this profile but is no longer desired"
            }
            Action::Warning => {
                "target was previously managed by this profile but drifted into non-symlink content"
            }
            _ => unreachable!(),
        };

        items.push(PlanItem {
            action,
            target,
            desired_source: None,
            reason: reason.to_string(),
        });
    }

    items.sort_by(|left, right| {
        let action_cmp = left.action.cmp(&right.action);
        if action_cmp == std::cmp::Ordering::Equal {
            path_to_string(&left.target).cmp(&path_to_string(&right.target))
        } else {
            action_cmp
        }
    });

    Ok(Plan {
        profile_name: resolved.profile_name,
        items,
        diagnostics: resolved.diagnostics,
    })
}

fn desired_source_map(plan: &Plan) -> BTreeMap<String, PathBuf> {
    let mut desired = BTreeMap::new();
    for item in &plan.items {
        if let Some(source) = &item.desired_source {
            desired.insert(path_to_string(&item.target), source.clone());
        }
    }
    desired
}

fn verify_plan_state(plan: &Plan, desired_sources: &BTreeMap<String, PathBuf>) -> Result<()> {
    for item in &plan.items {
        match item.action {
            Action::Create | Action::Update | Action::Skip => {
                if let Some(expected) = desired_sources.get(&path_to_string(&item.target)) {
                    let current = snapshot_path(&item.target)?;
                    if !symlink_matches_expected(&item.target, &current, expected) {
                        bail!(
                            "verification failed for {} after {}",
                            item.target.display(),
                            item.action.as_str()
                        );
                    }
                }
            }
            Action::Remove => {
                let current = snapshot_path(&item.target)?;
                if current != PathState::Missing {
                    bail!(
                        "verification failed: {} still exists after remove",
                        item.target.display()
                    );
                }
            }
            Action::Warning | Action::Danger => {}
        }
    }

    Ok(())
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Create => "create",
            Action::Update => "update",
            Action::Remove => "remove",
            Action::Skip => "skip",
            Action::Warning => "warning",
            Action::Danger => "danger",
        }
    }
}

impl DoctorIssueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DoctorIssueKind::BrokenSymlink => "broken_symlink",
            DoctorIssueKind::MissingManagedTarget => "missing_managed_target",
            DoctorIssueKind::ManagedDrift => "managed_drift",
        }
    }
}
