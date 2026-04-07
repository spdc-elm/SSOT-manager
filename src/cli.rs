use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::{load_config, load_editable_config};
use crate::inspection::{
    ProfileExplainView, ProfileListView, ProfileShowView, explain_profile, list_profiles,
    show_profile,
};
use crate::prompt::{
    CompositionListView, CompositionShowView, build_composition, list_compositions,
    preview_composition, show_composition,
};
use crate::reconcile::{
    Action, apply_plan, apply_plan_force_with_backup, build_plan, can_force_with_backup,
    doctor_profile, undo_last_apply,
};
use crate::state::StateStore;

#[derive(Debug, Parser)]
#[command(name = "ssot", about = "Deterministic SSOT asset manager")]
struct Cli {
    #[arg(long, global = true, default_value = "ssot.yaml")]
    config: PathBuf,
    #[arg(long, global = true)]
    state_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    Prompt {
        #[command(subcommand)]
        command: PromptCommand,
    },
    Tui,
    Undo,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Validate,
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    List {
        #[arg(long)]
        json: bool,
    },
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
    Explain {
        name: String,
        #[arg(long)]
        json: bool,
    },
    Plan {
        name: String,
    },
    Apply {
        name: String,
        #[arg(long)]
        force_with_backup: bool,
    },
    Doctor {
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum PromptCommand {
    List,
    Show { name: String },
    Preview { name: String },
    Build { name: String },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = StateStore::new(cli.state_dir)?;

    match cli.command {
        Commands::Config { command } => {
            let config = load_config(&cli.config)?;
            match command {
                ConfigCommand::Validate => {
                    let rule_count: usize = config
                        .profiles
                        .values()
                        .map(|profile| profile.rules.len())
                        .sum();
                    println!(
                        "Config is valid: version={} profiles={} rules={} compositions={}",
                        config.version,
                        config.profiles.len(),
                        rule_count,
                        config.compositions.len()
                    );
                }
            }
        }
        Commands::Profile { command } => {
            let config = load_config(&cli.config)?;
            let state = store.load()?;

            match command {
                ProfileCommand::List { json } => {
                    let view = list_profiles(&config);
                    if json {
                        print_json(&view)?;
                    } else {
                        print_profile_list(&view);
                    }
                }
                ProfileCommand::Show { name, json } => {
                    let view = show_profile(&config, &name)?;
                    if json {
                        print_json(&view)?;
                    } else {
                        print_profile_show(&view);
                    }
                }
                ProfileCommand::Explain { name, json } => {
                    let view = explain_profile(&config, &name, &state)?;
                    if json {
                        print_json(&view)?;
                    } else {
                        print_profile_explain(&view);
                    }
                }
                ProfileCommand::Plan { name } => {
                    let plan = build_plan(&config, &name, &state)?;
                    print_plan(&plan);
                }
                ProfileCommand::Apply {
                    name,
                    force_with_backup,
                } => {
                    let plan = build_plan(&config, &name, &state)?;
                    print_plan(&plan);
                    let result = if force_with_backup {
                        if can_force_with_backup(&plan) {
                            apply_plan_force_with_backup(plan, &state, &store)?
                        } else {
                            apply_plan_force_with_backup(plan, &state, &store)?
                        }
                    } else {
                        apply_plan(plan, &state, &store)?
                    };
                    println!(
                        "Applied profile '{}': {} journal entries written to {}",
                        result.plan.profile_name,
                        result.journal.entries.len(),
                        store.root().display()
                    );
                }
                ProfileCommand::Doctor { name } => {
                    let report = doctor_profile(&config, &name, &state)?;
                    if report.issues.is_empty() {
                        println!("Doctor OK for profile '{}'", report.profile_name);
                    } else {
                        println!(
                            "Doctor issues for profile '{}': {}",
                            report.profile_name,
                            report.issues.len()
                        );
                        for issue in report.issues {
                            println!(
                                "- {} {}: {}",
                                issue.kind.as_str(),
                                issue.target.display(),
                                issue.message
                            );
                        }
                    }
                }
            }
        }
        Commands::Prompt { command } => {
            let config = load_config(&cli.config)?;

            match command {
                PromptCommand::List => {
                    let view = list_compositions(&config);
                    print_prompt_list(&view);
                }
                PromptCommand::Show { name } => {
                    let view = show_composition(&config, &name)?;
                    print_prompt_show(&view);
                }
                PromptCommand::Preview { name } => {
                    print!("{}", preview_composition(&config, &name)?);
                }
                PromptCommand::Build { name } => {
                    let result = build_composition(&config, &name)?;
                    println!(
                        "Built composition '{}': {}",
                        result.composition_name,
                        result.output.display()
                    );
                }
            }
        }
        Commands::Tui => {
            let config = load_editable_config(&cli.config)?;
            crate::tui::run_tui(config, store)?;
        }
        Commands::Undo => {
            let result = undo_last_apply(&store)?;
            println!(
                "Undid last apply for profile '{}': {} targets reverted",
                result.profile_name,
                result.reverted_targets.len()
            );
        }
    }

    Ok(())
}

fn print_plan(plan: &crate::reconcile::Plan) {
    println!("Plan for profile '{}':", plan.profile_name);

    for diagnostic in &plan.diagnostics {
        println!("- warning [{}] {}", diagnostic.code, diagnostic.message);
    }

    for item in &plan.items {
        let action_label = action_label(item.action, item.forceable);
        match &item.desired_source {
            Some(source) => println!(
                "- {:<7} {} -> {} ({})",
                action_label,
                item.target.display(),
                source.display(),
                item.reason
            ),
            None => println!(
                "- {:<7} {} ({})",
                action_label,
                item.target.display(),
                item.reason
            ),
        }
    }

    let mut counts = BTreeMap::<&str, usize>::new();
    for item in &plan.items {
        *counts.entry(item.action.as_str()).or_default() += 1;
    }

    let summary = [
        Action::Create,
        Action::Update,
        Action::Remove,
        Action::Skip,
        Action::Warning,
        Action::Danger,
    ]
    .into_iter()
    .map(|action| {
        format!(
            "{}={}",
            action.as_str(),
            counts.get(action.as_str()).copied().unwrap_or(0)
        )
    })
    .collect::<Vec<_>>()
    .join(" ");

    println!("Summary: {summary}");
}

fn print_profile_list(view: &ProfileListView) {
    println!("Profiles:");
    for profile in &view.profiles {
        println!(
            "- {} source_root={} rules={} enabled={} disabled={}",
            profile.name,
            profile.source_root,
            profile.rule_count,
            profile.enabled_rule_count,
            profile.disabled_rule_count
        );
    }
}

fn print_profile_show(view: &ProfileShowView) {
    println!("Profile '{}':", view.profile_name);
    println!("- source_root={}", view.source_root);
    println!(
        "- rules={} enabled={} disabled={}",
        view.rule_count, view.enabled_rule_count, view.disabled_rule_count
    );
    if view.required_compositions.is_empty() {
        println!("- requires: none");
    } else {
        println!("- requires:");
        for requirement in &view.required_compositions {
            println!(
                "  - {} [{}] {} ({})",
                requirement.name, requirement.status, requirement.output, requirement.message
            );
        }
    }
    for rule in &view.rules {
        println!(
            "- rule {} select={} mode={} enabled={}",
            rule.index, rule.select, rule.mode, rule.enabled
        );
        for destination in &rule.destinations {
            println!("  to {}", destination);
        }
        if !rule.tags.is_empty() {
            println!("  tags {}", rule.tags.join(","));
        }
        if let Some(note) = &rule.note {
            println!("  note {}", note);
        }
    }
}

fn print_profile_explain(view: &ProfileExplainView) {
    println!("Explain profile '{}':", view.profile_name);
    println!("- source_root={}", view.source_root);
    if view.required_compositions.is_empty() {
        println!("Required compositions: none");
    } else {
        println!("Required compositions:");
        for requirement in &view.required_compositions {
            println!(
                "- {} [{}] {} ({})",
                requirement.name, requirement.status, requirement.output, requirement.message
            );
        }
    }

    if view.diagnostics.is_empty() {
        println!("Diagnostics: none");
    } else {
        println!("Diagnostics:");
        for diagnostic in &view.diagnostics {
            println!("- [{}] {}", diagnostic.code, diagnostic.message);
        }
    }

    if view.intents.is_empty() {
        println!("Resolved intents: none");
    } else {
        println!("Resolved intents:");
        for intent in &view.intents {
            println!("- {} -> {} ({})", intent.target, intent.source, intent.mode);
        }
    }

    let summary = [
        Action::Create,
        Action::Update,
        Action::Remove,
        Action::Skip,
        Action::Warning,
        Action::Danger,
    ]
    .into_iter()
    .map(|action| {
        format!(
            "{}={}",
            action.as_str(),
            view.plan_summary
                .get(action.as_str())
                .copied()
                .unwrap_or_default()
        )
    })
    .collect::<Vec<_>>()
    .join(" ");
    println!("Plan summary: {summary}");

    if view.plan_items.is_empty() {
        println!("Plan items: none");
    } else {
        println!("Plan items:");
        for item in &view.plan_items {
            let action_label = action_label_from_view(&item.action, item.forceable);
            match &item.desired_source {
                Some(source) => println!(
                    "- {:<7} {} -> {} ({})",
                    action_label, item.target, source, item.reason
                ),
                None => println!("- {:<7} {} ({})", action_label, item.target, item.reason),
            }
        }
    }
}

fn print_prompt_list(view: &CompositionListView) {
    if view.compositions.is_empty() {
        println!("No prompt compositions configured");
        return;
    }

    for composition in &view.compositions {
        println!(
            "- {} -> {} (inputs={})",
            composition.name, composition.output, composition.input_count
        );
    }
}

fn print_prompt_show(view: &CompositionShowView) {
    println!("Composition '{}':", view.composition_name);
    println!("- output={}", view.output);
    println!("- renderer={}", view.renderer_kind);
    println!(
        "- outer_wrapper before={:?} after={:?}",
        view.outer_wrapper.before, view.outer_wrapper.after
    );
    if view.variables.is_empty() {
        println!("- variables: none");
    } else {
        println!("- variables:");
        for (key, value) in &view.variables {
            println!("  - {}={}", key, value);
        }
    }
    if view.inputs.is_empty() {
        println!("- inputs: none");
    } else {
        println!("- inputs:");
        for input in &view.inputs {
            println!(
                "  - {} path={} before={:?} after={:?}",
                input.index, input.path, input.wrapper.before, input.wrapper.after
            );
        }
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn action_label(action: Action, forceable: bool) -> String {
    action_label_from_view(action.as_str(), forceable)
}

fn action_label_from_view(action: &str, forceable: bool) -> String {
    if action == "danger" && forceable {
        "danger*".to_string()
    } else {
        action.to_string()
    }
}
