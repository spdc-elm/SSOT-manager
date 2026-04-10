use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};

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
#[command(
    name = "ssot-manager",
    about = "Deterministic SSOT asset manager",
    after_help = "Examples:\n  ssot-manager config validate\n  ssot-manager profile list\n  ssot-manager profile explain <NAME>\n  ssot-manager profile apply <NAME>\n\nCommon workflow:\n  1. Use `profile explain` to inspect resolved source -> target intents.\n  2. Use `profile plan` to review concrete create/update/remove actions.\n  3. Use `profile apply` to execute the plan and write journal entries."
)]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "ssot.yaml",
        help = "Config file path"
    )]
    config: PathBuf,
    #[arg(
        long,
        global = true,
        help = "Override the state directory for journals and records"
    )]
    state_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(
        about = "Validate config files and referenced assets",
        after_help = "Example:\n  ssot-manager config validate"
    )]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(
        about = "Inspect, plan, apply, and diagnose profile syncs",
        after_help = "Workflow:\n  ssot-manager profile show <NAME>\n  ssot-manager profile explain <NAME>\n  ssot-manager profile plan <NAME>\n  ssot-manager profile apply <NAME>\n\nCommand roles:\n  show: inspect the declared profile definition.\n  explain: resolve prerequisites, source -> target intents, and plan summary without mutating the filesystem.\n  plan: show concrete create/update/remove/danger actions for the current state.\n  apply: execute the current plan and record journal entries.\n  doctor: detect drift, broken links, and ownership issues for managed targets.\n\nTarget path rules:\n  If `to` ends with `/`, already exists as a directory, or a rule matches multiple assets,\n  the source basename is appended to the destination.\n\n  Example: source_root=/repo, select=docs, to=/dest/sys1/ -> /dest/sys1/docs\n  Example: source_root=/repo/docs, select=*, to=/dest/sys1/ -> entries land directly under /dest/sys1/"
    )]
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    #[command(
        about = "Inspect and build prompt compositions",
        after_help = "Examples:\n  ssot-manager prompt list\n  ssot-manager prompt preview <NAME>\n  ssot-manager prompt build <NAME>"
    )]
    Prompt {
        #[command(subcommand)]
        command: PromptCommand,
    },
    #[command(about = "Open the interactive profile browser and editor")]
    Tui,
    #[command(about = "Undo the last successful profile apply journal")]
    Undo,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    #[command(about = "Validate config structure, paths, and cross-references")]
    Validate,
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    #[command(about = "List configured profiles and their effective source roots")]
    List {
        #[arg(
            long,
            help = "Emit machine-readable JSON instead of human-readable text"
        )]
        json: bool,
    },
    #[command(about = "Show the effective definition of a profile")]
    Show {
        #[arg(value_name = "NAME", help = "Profile name")]
        name: String,
        #[arg(
            long,
            help = "Emit machine-readable JSON instead of human-readable text"
        )]
        json: bool,
    },
    #[command(
        about = "Resolve source -> target intents and summarize the current plan without mutating the filesystem"
    )]
    Explain {
        #[arg(value_name = "NAME", help = "Profile name")]
        name: String,
        #[arg(
            long,
            help = "Emit machine-readable JSON instead of human-readable text"
        )]
        json: bool,
    },
    #[command(about = "Show concrete create/update/remove actions for a profile")]
    Plan {
        #[arg(value_name = "NAME", help = "Profile name")]
        name: String,
    },
    #[command(about = "Execute the current plan for a profile and record journal entries")]
    Apply {
        #[arg(value_name = "NAME", help = "Profile name")]
        name: String,
        #[arg(
            long,
            help = "Allow forceable danger actions by backing up the replaced unmanaged content first"
        )]
        force_with_backup: bool,
    },
    #[command(about = "Detect drift, broken links, and ownership issues for managed targets")]
    Doctor {
        #[arg(value_name = "NAME", help = "Profile name")]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum PromptCommand {
    #[command(about = "List configured prompt compositions")]
    List,
    #[command(about = "Show the effective recipe for a composition")]
    Show {
        #[arg(value_name = "NAME", help = "Composition name")]
        name: String,
    },
    #[command(about = "Render a composition without writing the output file")]
    Preview {
        #[arg(value_name = "NAME", help = "Composition name")]
        name: String,
    },
    #[command(about = "Write a composition output under source_root")]
    Build {
        #[arg(value_name = "NAME", help = "Composition name")]
        name: String,
    },
}

pub fn run() -> Result<()> {
    let cli = parse_cli_or_exit();
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

fn parse_cli_or_exit() -> Cli {
    let args: Vec<OsString> = std::env::args_os().collect();

    match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => exit_with_cli_error(error, &args),
    }
}

fn exit_with_cli_error(error: clap::Error, args: &[OsString]) -> ! {
    if error.kind() == ErrorKind::InvalidSubcommand {
        if let Some(tip) = misplaced_profile_subcommand_tip(args) {
            let rendered = error.to_string();
            eprint!("{rendered}");
            if !rendered.ends_with('\n') {
                eprintln!();
            }
            eprintln!("tip: {tip}");
            std::process::exit(error.exit_code());
        }
    }

    error.exit()
}

fn misplaced_profile_subcommand_tip(args: &[OsString]) -> Option<&'static str> {
    match first_non_option_arg(args)? {
        "list" => Some("did you mean 'ssot-manager profile list'?"),
        "show" => Some("did you mean 'ssot-manager profile show <NAME>'?"),
        "explain" => Some("did you mean 'ssot-manager profile explain <NAME>'?"),
        "plan" => Some("did you mean 'ssot-manager profile plan <NAME>'?"),
        "apply" => Some("did you mean 'ssot-manager profile apply <NAME>'?"),
        "doctor" => Some("did you mean 'ssot-manager profile doctor <NAME>'?"),
        _ => None,
    }
}

fn first_non_option_arg(args: &[OsString]) -> Option<&str> {
    let mut skip_next_value = false;

    for arg in args.iter().skip(1) {
        let arg = arg.to_str()?;
        if skip_next_value {
            skip_next_value = false;
            continue;
        }

        match arg {
            "--config" | "--state-dir" => {
                skip_next_value = true;
            }
            "--" => return None,
            _ if arg.starts_with("--config=") || arg.starts_with("--state-dir=") => {}
            _ if arg.starts_with('-') => {}
            _ => return Some(arg),
        }
    }

    None
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
