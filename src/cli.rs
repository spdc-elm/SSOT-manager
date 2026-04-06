use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::load_config;
use crate::inspection::{
    ProfileExplainView, ProfileListView, ProfileShowView, explain_profile, list_profiles,
    show_profile,
};
use crate::reconcile::{Action, build_plan, doctor_profile, undo_last_apply};
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
    },
    Doctor {
        name: String,
    },
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
                        "Config is valid: version={} profiles={} rules={}",
                        config.version,
                        config.profiles.len(),
                        rule_count
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
                ProfileCommand::Apply { name } => {
                    let plan = build_plan(&config, &name, &state)?;
                    print_plan(&plan);
                    let result = crate::reconcile::apply_plan(plan, &state, &store)?;
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
        Commands::Tui => {
            let config = load_config(&cli.config)?;
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
        match &item.desired_source {
            Some(source) => println!(
                "- {:<7} {} -> {} ({})",
                item.action.as_str(),
                item.target.display(),
                source.display(),
                item.reason
            ),
            None => println!(
                "- {:<7} {} ({})",
                item.action.as_str(),
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
            match &item.desired_source {
                Some(source) => println!(
                    "- {:<7} {} -> {} ({})",
                    item.action, item.target, source, item.reason
                ),
                None => println!("- {:<7} {} ({})", item.action, item.target, item.reason),
            }
        }
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
