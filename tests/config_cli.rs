use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("ssot-manager").expect("binary should build")
}

#[test]
fn validates_a_valid_config_fixture() {
    let fixture = prepare_fixture("valid.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
    });

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("config")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicates::str::contains("Config is valid"));
}

#[test]
fn rejects_unsupported_mode_fixture() {
    let fixture = prepare_fixture("unsupported-mode.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
    });

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("config")
        .arg("validate")
        .assert()
        .failure()
        .stderr(predicates::str::contains("only symlink is supported"));
}

#[test]
fn ignores_disabled_rules_when_planning() {
    let fixture = prepare_fixture("disabled-rule.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Skills/beta")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("profile")
        .arg("plan")
        .arg("main")
        .assert()
        .success()
        .stdout(predicates::str::contains("alpha"))
        .stdout(predicates::str::contains("beta").not());
}

#[test]
fn rejects_unknown_profile() {
    let fixture = prepare_fixture("valid.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
    });

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("profile")
        .arg("plan")
        .arg("missing")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown profile"));
}

#[test]
fn profile_source_root_overrides_global_source_root() {
    let fixture = prepare_fixture("profile-source-root.yaml", |temp| {
        fs::create_dir_all(temp.path().join("global-source/Skills/global-only")).unwrap();
        fs::create_dir_all(temp.path().join("profile-source/Skills/profile-only")).unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("profile")
        .arg("plan")
        .arg("override-root")
        .assert()
        .success()
        .stdout(predicates::str::contains("profile-only"))
        .stdout(predicates::str::contains("global-only").not());
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
            "__GLOBAL_SOURCE_ROOT__",
            &temp.path().join("global-source").display().to_string(),
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

    #[allow(dead_code)]
    fn root(&self) -> &Path {
        self.temp.path()
    }
}
