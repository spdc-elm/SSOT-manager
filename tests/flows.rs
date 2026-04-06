use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("ssot-manager").expect("binary should build")
}

#[test]
fn plan_reports_create_skip_and_danger() {
    let harness = Harness::new();
    fs::create_dir_all(harness.dest_root().join("skills")).unwrap();
    fs::create_dir_all(harness.dest_root().join("manual")).unwrap();
    fs::write(harness.dest_root().join("manual/notes.md"), "keep me").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        harness.source_root().join("Skills/alpha"),
        harness.dest_root().join("skills/alpha"),
    )
    .unwrap();

    let output = bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("profile")
        .arg("plan")
        .arg("skill-global")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("create"));
    assert!(stdout.contains("skip"));
    assert!(stdout.contains("danger"));
    assert!(stdout.contains("manual/notes.md"));
}

#[test]
fn apply_doctor_and_undo_cover_the_main_reconcile_loop() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("skill-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("Applied profile"));

    let skill_target = harness.dest_root().join("safe-skills/alpha");
    assert!(
        fs::symlink_metadata(&skill_target)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let removed_source = harness.source_root().join("Skills/alpha");
    fs::remove_dir_all(&removed_source).unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("doctor")
        .arg("skill-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("broken_symlink"));

    fs::create_dir_all(&removed_source).unwrap();
    fs::write(removed_source.join("SKILL.md"), "# restored").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("undo")
        .assert()
        .success()
        .stdout(predicates::str::contains("targets reverted"));

    assert!(!skill_target.exists());
}

#[test]
fn apply_refuses_unmanaged_collision() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    let colliding_target = harness.dest_root().join("manual/notes.md");
    if let Some(parent) = colliding_target.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&colliding_target, "manual").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("skill-global")
        .assert()
        .failure()
        .stderr(predicates::str::contains("danger actions"));
}

struct Harness {
    temp: TempDir,
    config_path: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let source_root = temp.path().join("source");
        let dest_root = temp.path().join("dest");

        fs::create_dir_all(source_root.join("Skills/alpha")).unwrap();
        fs::create_dir_all(source_root.join("Skills/beta")).unwrap();
        fs::create_dir_all(source_root.join("Notes")).unwrap();
        fs::write(source_root.join("Skills/alpha/SKILL.md"), "# alpha").unwrap();
        fs::write(source_root.join("Skills/beta/SKILL.md"), "# beta").unwrap();
        fs::write(source_root.join("Notes/notes.md"), "notes").unwrap();
        fs::create_dir_all(&dest_root).unwrap();

        let template = fs::read_to_string(Path::new("tests/fixtures/flow-config.yaml")).unwrap();
        let config = template
            .replace("__SOURCE_ROOT__", &source_root.display().to_string())
            .replace("__DEST_ROOT__", &dest_root.display().to_string());
        let config_path = temp.path().join("config.yaml");
        fs::write(&config_path, config).unwrap();

        Self { temp, config_path }
    }

    fn path(&self) -> &Path {
        self.temp.path()
    }

    fn source_root(&self) -> PathBuf {
        self.temp.path().join("source")
    }

    fn dest_root(&self) -> PathBuf {
        self.temp.path().join("dest")
    }

    fn config_path(&self) -> &Path {
        &self.config_path
    }
}
