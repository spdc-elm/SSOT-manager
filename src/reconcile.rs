use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::config::{
    Config, ConfigDiagnostic, MaterializationMode, ResolvedProfile, SyncIntent, resolve_profile,
};
use crate::paths::path_to_string;
use crate::prompt::profile_requirements;
use crate::state::{
    ApplyJournal, JournalEntry, ManagedState, PathState, StateStore, build_record,
    create_backup_artifact, materialize_target, now_timestamp, remove_existing_path,
    restore_from_backup, restore_from_source, restore_path, snapshot_path, target_matches_source,
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
    pub desired_mode: Option<MaterializationMode>,
    pub forceable: bool,
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
    let prerequisite_items = blocking_prerequisite_items(config, profile_name)?;
    if !prerequisite_items.is_empty() {
        return Ok(Plan {
            profile_name: profile_name.to_string(),
            items: prerequisite_items,
            diagnostics: Vec::new(),
        });
    }

    let resolved = resolve_profile(config, profile_name)?;
    plan_from_resolved(resolved, state)
}

pub fn apply_plan(plan: Plan, state: &ManagedState, store: &StateStore) -> Result<ApplyResult> {
    apply_plan_internal(plan, state, store, false)
}

pub fn apply_plan_force_with_backup(
    plan: Plan,
    state: &ManagedState,
    store: &StateStore,
) -> Result<ApplyResult> {
    apply_plan_internal(plan, state, store, true)
}

fn apply_plan_internal(
    plan: Plan,
    state: &ManagedState,
    store: &StateStore,
    force_with_backup: bool,
) -> Result<ApplyResult> {
    if plan
        .items
        .iter()
        .any(|item| item.action == Action::Danger && (!force_with_backup || !item.forceable))
    {
        if force_with_backup {
            bail!(
                "refusing force-with-backup apply because the plan contains non-forceable danger actions"
            );
        }
        bail!("refusing to apply because the plan contains danger actions");
    }

    let timestamp = now_timestamp()?;
    let desired_targets = desired_target_map(&plan);
    let mut next_state = state.clone();
    let mut journal_entries = Vec::new();

    for (index, item) in plan.items.iter().enumerate() {
        match item.action {
            Action::Create | Action::Update => {
                let desired_source = item
                    .desired_source
                    .as_ref()
                    .expect("desired source is required for create/update/force");
                let desired_mode = item
                    .desired_mode
                    .expect("desired mode is required for create/update/force");
                let before = snapshot_path(&item.target)?;
                let record_before = next_state
                    .records
                    .get(&path_to_string(&item.target))
                    .cloned();
                let record_before_source_state = record_before
                    .as_ref()
                    .and_then(|record| match record.mode {
                        MaterializationMode::Copy | MaterializationMode::Hardlink => {
                            Some(snapshot_path(Path::new(&record.source)))
                        }
                        MaterializationMode::Symlink => None,
                    })
                    .transpose()?;
                let backup_before = if item.action == Action::Danger {
                    create_backup_artifact(store, timestamp, index, &item.target, &before)?
                } else {
                    None
                };

                remove_existing_path(&item.target)?;
                materialize_target(desired_source, &item.target, desired_mode)?;

                let after = snapshot_path(&item.target)?;
                if !target_matches_source(desired_source, &item.target, &after, desired_mode)? {
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
                        mode: desired_mode,
                    },
                    timestamp,
                ));

                if let Some(record) = &record_after {
                    next_state
                        .records
                        .insert(path_to_string(&item.target), record.clone());
                }

                journal_entries.push(JournalEntry {
                    action: if item.action == Action::Danger {
                        "force_overwrite".to_string()
                    } else {
                        item.action.as_str().to_string()
                    },
                    target: path_to_string(&item.target),
                    before,
                    after,
                    record_before,
                    record_before_source_state,
                    backup_before,
                    record_after,
                });
            }
            Action::Danger if item.forceable => {
                let desired_source = item
                    .desired_source
                    .as_ref()
                    .expect("desired source is required for create/update/force");
                let desired_mode = item
                    .desired_mode
                    .expect("desired mode is required for create/update/force");
                let before = snapshot_path(&item.target)?;
                let record_before = next_state
                    .records
                    .get(&path_to_string(&item.target))
                    .cloned();
                let record_before_source_state = record_before
                    .as_ref()
                    .and_then(|record| match record.mode {
                        MaterializationMode::Copy | MaterializationMode::Hardlink => {
                            Some(snapshot_path(Path::new(&record.source)))
                        }
                        MaterializationMode::Symlink => None,
                    })
                    .transpose()?;
                let backup_before =
                    create_backup_artifact(store, timestamp, index, &item.target, &before)?;

                remove_existing_path(&item.target)?;
                materialize_target(desired_source, &item.target, desired_mode)?;

                let after = snapshot_path(&item.target)?;
                if !target_matches_source(desired_source, &item.target, &after, desired_mode)? {
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
                        mode: desired_mode,
                    },
                    timestamp,
                ));

                if let Some(record) = &record_after {
                    next_state
                        .records
                        .insert(path_to_string(&item.target), record.clone());
                }

                journal_entries.push(JournalEntry {
                    action: "force_overwrite".to_string(),
                    target: path_to_string(&item.target),
                    before,
                    after,
                    record_before,
                    record_before_source_state,
                    backup_before,
                    record_after,
                });
            }
            Action::Remove => {
                let before = snapshot_path(&item.target)?;
                let record_before = next_state
                    .records
                    .get(&path_to_string(&item.target))
                    .cloned();
                let record_before_source_state = record_before
                    .as_ref()
                    .and_then(|record| match record.mode {
                        MaterializationMode::Copy | MaterializationMode::Hardlink => {
                            Some(snapshot_path(Path::new(&record.source)))
                        }
                        MaterializationMode::Symlink => None,
                    })
                    .transpose()?;

                match &before {
                    PathState::Missing => {}
                    PathState::Symlink { .. } => remove_existing_path(&item.target)?,
                    PathState::File { .. } | PathState::Directory { .. } => {
                        match record_before.as_ref().map(|record| record.mode) {
                            Some(MaterializationMode::Copy | MaterializationMode::Hardlink) => {
                                remove_existing_path(&item.target)?
                            }
                            _ => bail!(
                                "refusing to remove '{}' because it drifted away from a managed symlink",
                                item.target.display()
                            ),
                        }
                    }
                    PathState::Other => bail!(
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
                    record_before_source_state,
                    backup_before: None,
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
                        if let Some(desired_mode) = item.desired_mode {
                            existing.mode = desired_mode;
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

    verify_plan_state(&plan, &desired_targets, force_with_backup)?;

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
            PathState::Symlink { .. } | PathState::File { .. } | PathState::Directory { .. } => {
                let source = PathBuf::from(&record.source);
                if matches!(record.mode, MaterializationMode::Symlink) && !source.exists() {
                    issues.push(DoctorIssue {
                        kind: DoctorIssueKind::BrokenSymlink,
                        target,
                        message: format!(
                            "managed symlink points to missing path {}",
                            source.display()
                        ),
                    });
                } else if !target_matches_source(
                    Path::new(&record.source),
                    &target,
                    &current,
                    record.mode,
                )? {
                    let message = match record.mode {
                        MaterializationMode::Symlink => {
                            format!("managed symlink no longer points to {}", source.display())
                        }
                        MaterializationMode::Copy => format!(
                            "managed copied target no longer matches source {}",
                            record.source
                        ),
                        MaterializationMode::Hardlink => format!(
                            "managed hardlinked target no longer matches source {}",
                            record.source
                        ),
                    };
                    issues.push(DoctorIssue {
                        kind: DoctorIssueKind::ManagedDrift,
                        target,
                        message,
                    });
                }
            }
            PathState::Other => issues.push(DoctorIssue {
                kind: DoctorIssueKind::ManagedDrift,
                target,
                message: "managed target was replaced by unsupported content".to_string(),
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
        match &entry.before {
            PathState::Missing | PathState::Symlink { .. } => restore_path(&target, &entry.before)?,
            PathState::File { .. } | PathState::Directory { .. } => {
                if let Some(backup_before) = &entry.backup_before {
                    restore_from_backup(&target, backup_before)?;
                } else {
                    let record_before = entry.record_before.as_ref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "undo cannot restore concrete content at {} without a managed record",
                            target.display()
                        )
                    })?;
                    let source_state = entry.record_before_source_state.as_ref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "undo cannot restore {} because the previous source snapshot is missing",
                            target.display()
                        )
                    })?;
                    restore_from_source(
                        &target,
                        Path::new(&record_before.source),
                        record_before.mode,
                        source_state,
                    )?;
                }
            }
            PathState::Other => bail!(
                "undo cannot restore unsupported content at {}",
                target.display()
            ),
        }

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

pub(crate) fn build_plan_from_resolved(
    resolved: ResolvedProfile,
    state: &ManagedState,
) -> Result<Plan> {
    plan_from_resolved(resolved, state)
}

fn blocking_prerequisite_items(config: &Config, profile_name: &str) -> Result<Vec<PlanItem>> {
    let requirements = profile_requirements(config, profile_name)?;

    Ok(requirements
        .into_iter()
        .filter(|requirement| requirement.status != "ready")
        .map(|requirement| PlanItem {
            action: Action::Danger,
            target: PathBuf::from(requirement.output),
            desired_source: None,
            desired_mode: None,
            forceable: false,
            reason: format!(
                "required composition '{}' is {} ({})",
                requirement.name, requirement.status, requirement.message
            ),
        })
        .collect())
}

fn plan_from_resolved(resolved: ResolvedProfile, state: &ManagedState) -> Result<Plan> {
    let mut items = Vec::new();
    let mut desired_targets = BTreeSet::new();

    for intent in &resolved.intents {
        let current = snapshot_path(&intent.target)?;
        let target_key = path_to_string(&intent.target);
        let record = state.records.get(&target_key);
        desired_targets.insert(target_key.clone());

        let item = if target_matches_source(&intent.source, &intent.target, &current, intent.mode)?
        {
            PlanItem {
                action: Action::Skip,
                target: intent.target.clone(),
                desired_source: Some(intent.source.clone()),
                desired_mode: Some(intent.mode),
                forceable: false,
                reason: "target already matches the desired materialization".to_string(),
            }
        } else {
            match &current {
                PathState::Missing => PlanItem {
                    action: Action::Create,
                    target: intent.target.clone(),
                    desired_source: Some(intent.source.clone()),
                    desired_mode: Some(intent.mode),
                    forceable: false,
                    reason: "target does not exist".to_string(),
                },
                PathState::Symlink { .. } => match record {
                    Some(record) if record.profile == resolved.profile_name => PlanItem {
                        action: Action::Update,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        desired_mode: Some(intent.mode),
                        forceable: false,
                        reason: "managed target no longer matches the desired materialization"
                            .to_string(),
                    },
                    Some(_) => PlanItem {
                        action: Action::Danger,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        desired_mode: Some(intent.mode),
                        forceable: false,
                        reason: "target is managed by another profile".to_string(),
                    },
                    None => PlanItem {
                        action: Action::Danger,
                        target: intent.target.clone(),
                        desired_source: Some(intent.source.clone()),
                        desired_mode: Some(intent.mode),
                        forceable: true,
                        reason: "target is an unmanaged symlink that would be replaced".to_string(),
                    },
                },
                PathState::File { .. } | PathState::Directory { .. } | PathState::Other => {
                    match record {
                        Some(record) if record.profile == resolved.profile_name => {
                            let action = match (intent.mode, &current) {
                                (
                                    MaterializationMode::Symlink,
                                    PathState::File { .. }
                                    | PathState::Directory { .. }
                                    | PathState::Other,
                                ) => Action::Warning,
                                (_, PathState::Other) => Action::Warning,
                                _ => Action::Update,
                            };
                            let reason = match action {
                                Action::Update => {
                                    "managed target no longer matches the desired materialization"
                                }
                                Action::Warning => {
                                    "managed target drifted into content that should be inspected before mutating"
                                }
                                _ => unreachable!(),
                            };
                            PlanItem {
                                action,
                                target: intent.target.clone(),
                                desired_source: Some(intent.source.clone()),
                                desired_mode: Some(intent.mode),
                                forceable: false,
                                reason: reason.to_string(),
                            }
                        }
                        Some(_) => PlanItem {
                            action: Action::Danger,
                            target: intent.target.clone(),
                            desired_source: Some(intent.source.clone()),
                            desired_mode: Some(intent.mode),
                            forceable: false,
                            reason: "target is managed by another profile".to_string(),
                        },
                        None => PlanItem {
                            action: Action::Danger,
                            target: intent.target.clone(),
                            desired_source: Some(intent.source.clone()),
                            desired_mode: Some(intent.mode),
                            forceable: !matches!(current, PathState::Other),
                            reason: "target contains unmanaged content".to_string(),
                        },
                    }
                }
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
        let action = match (&record.mode, current) {
            (_, PathState::Missing) => Action::Remove,
            (MaterializationMode::Symlink, PathState::Symlink { .. }) => Action::Remove,
            (
                MaterializationMode::Copy | MaterializationMode::Hardlink,
                PathState::Symlink { .. },
            )
            | (
                MaterializationMode::Copy | MaterializationMode::Hardlink,
                PathState::File { .. } | PathState::Directory { .. },
            ) => Action::Remove,
            _ => Action::Warning,
        };
        let reason = match action {
            Action::Remove => {
                "target was previously managed by this profile but is no longer desired"
            }
            Action::Warning => {
                "target was previously managed by this profile but drifted into content that should be inspected before removal"
            }
            _ => unreachable!(),
        };

        items.push(PlanItem {
            action,
            target,
            desired_source: None,
            desired_mode: None,
            forceable: false,
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

fn desired_target_map(plan: &Plan) -> BTreeMap<String, (PathBuf, MaterializationMode)> {
    let mut desired = BTreeMap::new();
    for item in &plan.items {
        if let (Some(source), Some(mode)) = (&item.desired_source, item.desired_mode) {
            desired.insert(path_to_string(&item.target), (source.clone(), mode));
        }
    }
    desired
}

fn verify_plan_state(
    plan: &Plan,
    desired_targets: &BTreeMap<String, (PathBuf, MaterializationMode)>,
    force_with_backup: bool,
) -> Result<()> {
    for item in &plan.items {
        match item.action {
            Action::Create | Action::Update | Action::Skip => {
                if let Some((expected_source, expected_mode)) =
                    desired_targets.get(&path_to_string(&item.target))
                {
                    let current = snapshot_path(&item.target)?;
                    if !target_matches_source(
                        expected_source,
                        &item.target,
                        &current,
                        *expected_mode,
                    )? {
                        bail!(
                            "verification failed for {} after {}",
                            item.target.display(),
                            item.action.as_str()
                        );
                    }
                }
            }
            Action::Danger if force_with_backup && item.forceable => {
                if let Some((expected_source, expected_mode)) =
                    desired_targets.get(&path_to_string(&item.target))
                {
                    let current = snapshot_path(&item.target)?;
                    if !target_matches_source(
                        expected_source,
                        &item.target,
                        &current,
                        *expected_mode,
                    )? {
                        bail!(
                            "verification failed for {} after force_overwrite",
                            item.target.display()
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

pub fn can_force_with_backup(plan: &Plan) -> bool {
    let mut has_danger = false;
    for item in &plan.items {
        if item.action == Action::Danger {
            has_danger = true;
            if !item.forceable {
                return false;
            }
        }
    }

    has_danger
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
