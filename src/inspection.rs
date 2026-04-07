use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::config::{Config, MaterializationMode, resolve_profile};
use crate::paths::path_to_string;
use crate::prompt::CompositionRequirementView;
use crate::prompt::profile_requirements;
use crate::reconcile::{Action, build_plan, build_plan_from_resolved};
use crate::state::ManagedState;

#[derive(Debug, Clone, Serialize)]
pub struct ProfileListView {
    pub profiles: Vec<ProfileSummaryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummaryView {
    pub name: String,
    pub source_root: String,
    pub rule_count: usize,
    pub enabled_rule_count: usize,
    pub disabled_rule_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileShowView {
    pub profile_name: String,
    pub source_root: String,
    pub rule_count: usize,
    pub enabled_rule_count: usize,
    pub disabled_rule_count: usize,
    pub required_compositions: Vec<CompositionRequirementView>,
    pub rules: Vec<RuleView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleView {
    pub index: usize,
    pub select: String,
    pub destinations: Vec<String>,
    pub mode: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileExplainView {
    pub profile_name: String,
    pub source_root: String,
    pub required_compositions: Vec<CompositionRequirementView>,
    pub diagnostics: Vec<DiagnosticView>,
    pub intents: Vec<IntentView>,
    pub plan_summary: BTreeMap<String, usize>,
    pub plan_items: Vec<PlanItemView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticView {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntentView {
    pub source: String,
    pub target: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanItemView {
    pub action: String,
    pub target: String,
    pub desired_source: Option<String>,
    pub forceable: bool,
    pub reason: String,
}

pub fn list_profiles(config: &Config) -> ProfileListView {
    let profiles = config
        .profiles
        .iter()
        .map(|(name, profile)| {
            let enabled_rule_count = profile.rules.iter().filter(|rule| rule.enabled).count();
            let disabled_rule_count = profile.rules.len() - enabled_rule_count;

            ProfileSummaryView {
                name: name.clone(),
                source_root: path_to_string(&profile.source_root),
                rule_count: profile.rules.len(),
                enabled_rule_count,
                disabled_rule_count,
            }
        })
        .collect();

    ProfileListView { profiles }
}

pub fn show_profile(config: &Config, profile_name: &str) -> Result<ProfileShowView> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow!("unknown profile '{profile_name}'"))?;
    let enabled_rule_count = profile.rules.iter().filter(|rule| rule.enabled).count();
    let disabled_rule_count = profile.rules.len() - enabled_rule_count;

    let rules = profile
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| RuleView {
            index: index + 1,
            select: rule.select.clone(),
            destinations: rule.to.clone(),
            mode: materialization_mode_as_str(rule.mode).to_string(),
            enabled: rule.enabled,
            tags: rule.tags.clone(),
            note: rule.note.clone(),
        })
        .collect();
    let required_compositions = profile_requirements(config, profile_name)?;

    Ok(ProfileShowView {
        profile_name: profile_name.to_string(),
        source_root: path_to_string(&profile.source_root),
        rule_count: profile.rules.len(),
        enabled_rule_count,
        disabled_rule_count,
        required_compositions,
        rules,
    })
}

pub fn explain_profile(
    config: &Config,
    profile_name: &str,
    state: &ManagedState,
) -> Result<ProfileExplainView> {
    let source_root = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow!("unknown profile '{profile_name}'"))?
        .source_root
        .clone();
    let required_compositions = profile_requirements(config, profile_name)?;
    let has_blocking_prerequisite = required_compositions
        .iter()
        .any(|composition| composition.status != "ready");
    let (diagnostics, intents, plan) = if has_blocking_prerequisite {
        (
            Vec::new(),
            Vec::new(),
            build_plan(config, profile_name, state)?,
        )
    } else {
        let resolved = resolve_profile(config, profile_name)?;
        let diagnostics = resolved
            .diagnostics
            .iter()
            .map(|diagnostic| DiagnosticView {
                code: diagnostic.code.to_string(),
                message: diagnostic.message.clone(),
            })
            .collect();
        let intents = resolved
            .intents
            .iter()
            .map(|intent| IntentView {
                source: path_to_string(&intent.source),
                target: path_to_string(&intent.target),
                mode: materialization_mode_as_str(intent.mode).to_string(),
            })
            .collect();
        let plan = build_plan_from_resolved(resolved, state)?;
        (diagnostics, intents, plan)
    };
    let mut plan_summary = BTreeMap::new();

    for action in [
        Action::Create,
        Action::Update,
        Action::Remove,
        Action::Skip,
        Action::Warning,
        Action::Danger,
    ] {
        plan_summary.insert(action.as_str().to_string(), 0);
    }

    let plan_items = plan
        .items
        .into_iter()
        .map(|item| {
            *plan_summary
                .entry(item.action.as_str().to_string())
                .or_default() += 1;
            PlanItemView {
                action: item.action.as_str().to_string(),
                target: path_to_string(&item.target),
                desired_source: item.desired_source.map(|source| path_to_string(&source)),
                forceable: item.forceable,
                reason: item.reason,
            }
        })
        .collect();

    Ok(ProfileExplainView {
        profile_name: profile_name.to_string(),
        source_root: path_to_string(&source_root),
        required_compositions,
        diagnostics,
        intents,
        plan_summary,
        plan_items,
    })
}

fn materialization_mode_as_str(mode: MaterializationMode) -> &'static str {
    mode.as_str()
}
