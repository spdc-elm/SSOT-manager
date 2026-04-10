use assert_cmd::Command;

fn bin() -> Command {
    Command::cargo_bin("ssot-manager").expect("binary should build")
}

#[test]
fn top_level_help_lists_command_descriptions_and_examples() {
    let stdout = command_stdout(bin().arg("--help").assert().success());

    assert!(stdout.contains("Validate config files and referenced assets"));
    assert!(stdout.contains("Inspect, plan, apply, and diagnose profile syncs"));
    assert!(stdout.contains("Inspect and build prompt compositions"));
    assert!(stdout.contains("Examples:"));
    assert!(stdout.contains("ssot-manager profile explain <NAME>"));
    assert!(stdout.contains("Use `profile plan` to review concrete create/update/remove actions."));
}

#[test]
fn profile_help_explains_command_roles_and_target_path_rules() {
    let stdout = command_stdout(bin().arg("profile").arg("--help").assert().success());

    assert!(stdout.contains("Show the effective definition of a profile"));
    assert!(stdout.contains(
        "Resolve source -> target intents and summarize the current plan without mutating the filesystem"
    ));
    assert!(stdout.contains("Show concrete create/update/remove actions for a profile"));
    assert!(stdout.contains("Command roles:"));
    assert!(stdout.contains("show: inspect the declared profile definition."));
    assert!(stdout.contains("`to` ends with `/`"));
    assert!(stdout.contains("the source basename is appended to the destination"));
    assert!(stdout.contains("source_root=/repo, select=docs, to=/dest/sys1/ -> /dest/sys1/docs"));
}

#[test]
fn misplaced_top_level_apply_suggests_profile_apply() {
    let stderr = command_stderr(bin().arg("apply").arg("--help").assert().failure());

    assert!(stderr.contains("unrecognized subcommand 'apply'"));
    assert!(stderr.contains("tip: did you mean 'ssot-manager profile apply <NAME>'?"));
}

fn command_stdout(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

fn command_stderr(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).unwrap()
}
