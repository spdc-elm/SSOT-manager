use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("ssot-manager").expect("binary should build")
}

#[test]
fn profile_list_reports_profiles_in_deterministic_order() {
    let fixture = prepare_fixture("inspection.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Skills/beta")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::create_dir_all(temp.path().join("profile-source/Skills/profile-only")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    let stdout = command_stdout(
        bin()
            .arg("--config")
            .arg(fixture.config_path())
            .arg("profile")
            .arg("list"),
    );

    let alpha_index = stdout.find("alpha").expect("alpha should be listed");
    let beta_index = stdout.find("beta").expect("beta should be listed");

    assert!(alpha_index < beta_index);
    assert!(stdout.contains("enabled=2 disabled=1"));
}

#[test]
fn profile_show_reports_effective_definition() {
    let fixture = prepare_fixture("inspection.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Skills/beta")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::create_dir_all(temp.path().join("profile-source/Skills/profile-only")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    let stdout = command_stdout(
        bin()
            .arg("--config")
            .arg(fixture.config_path())
            .arg("profile")
            .arg("show")
            .arg("alpha"),
    );

    assert!(stdout.contains("Profile 'alpha':"));
    assert!(stdout.contains(&format!(
        "source_root={}",
        fixture.root().join("source").display()
    )));
    assert!(stdout.contains("rules=3 enabled=2 disabled=1"));
    assert!(stdout.contains("rule 1 select=Skills/* mode=symlink enabled=true"));
    assert!(stdout.contains("rule 3 select=Agents/assistant.md mode=symlink enabled=false"));
    assert!(stdout.contains("tags core,global"));
    assert!(stdout.contains("note sync skills"));
}

#[test]
fn profile_show_json_reports_override_source_root() {
    let fixture = prepare_fixture("inspection.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Skills/beta")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::create_dir_all(temp.path().join("profile-source/Skills/profile-only")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    let stdout = command_stdout(
        bin()
            .arg("--config")
            .arg(fixture.config_path())
            .arg("profile")
            .arg("show")
            .arg("beta")
            .arg("--json"),
    );
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["profile_name"], "beta");
    assert_eq!(
        value["source_root"],
        fixture.root().join("profile-source").display().to_string()
    );
    assert_eq!(value["rules"].as_array().unwrap().len(), 1);
}

#[test]
fn profile_explain_reports_diagnostics_and_plan_summary() {
    let fixture = prepare_fixture("inspection.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Skills/beta")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::create_dir_all(temp.path().join("profile-source/Skills/profile-only")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    let stdout = command_stdout(
        bin()
            .arg("--config")
            .arg(fixture.config_path())
            .arg("profile")
            .arg("explain")
            .arg("alpha"),
    );

    assert!(stdout.contains("Explain profile 'alpha':"));
    assert!(stdout.contains("[asset_not_found]"));
    assert!(stdout.contains("Resolved intents:"));
    assert!(stdout.contains("Plan summary: create=2"));
    assert!(stdout.contains("/skills/alpha"));
    assert!(stdout.contains("/skills/beta"));
}

#[test]
fn profile_explain_json_includes_plan_items() {
    let fixture = prepare_fixture("inspection.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Skills/beta")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::create_dir_all(temp.path().join("profile-source/Skills/profile-only")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    let stdout = command_stdout(
        bin()
            .arg("--config")
            .arg(fixture.config_path())
            .arg("profile")
            .arg("explain")
            .arg("alpha")
            .arg("--json"),
    );
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["profile_name"], "alpha");
    assert_eq!(value["plan_summary"]["create"], 2);
    assert_eq!(value["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(value["plan_items"].as_array().unwrap().len(), 2);
}

fn command_stdout(command: &mut Command) -> String {
    let output = command.assert().success().get_output().stdout.clone();
    String::from_utf8(output).unwrap()
}

fn prepare_fixture(name: &str, setup: impl FnOnce(&TempDir)) -> Fixture {
    let temp = TempDir::new().unwrap();
    setup(&temp);

    let fixture_template =
        fs::read_to_string(Path::new("tests/fixtures").join(name)).expect("fixture should exist");
    let contents = fixture_template
        .replace(
            "__SOURCE_ROOT__",
            &temp.path().join("source").display().to_string(),
        )
        .replace(
            "__PROFILE_SOURCE_ROOT__",
            &temp.path().join("profile-source").display().to_string(),
        )
        .replace(
            "__DEST_ROOT__",
            &temp.path().join("dest").display().to_string(),
        );
    let config_path = temp.path().join("config.yaml");
    fs::write(&config_path, contents).unwrap();

    Fixture { temp, config_path }
}

struct Fixture {
    temp: TempDir,
    config_path: PathBuf,
}

impl Fixture {
    fn config_path(&self) -> &Path {
        &self.config_path
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }
}
