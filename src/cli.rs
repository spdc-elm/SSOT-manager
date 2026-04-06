use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::load_config;
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
    Undo,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Validate,
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    Plan { name: String },
    Apply { name: String },
    Doctor { name: String },
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
