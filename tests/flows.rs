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

#[test]
fn copy_mode_updates_plan_reports_drift_and_blocks_undo_after_target_edit() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("copy-safe")
        .assert()
        .success();

    let source_file = harness.source_root().join("Skills/alpha/SKILL.md");
    let target_file = harness.dest_root().join("copied-skills/alpha/SKILL.md");

    assert!(
        fs::symlink_metadata(&target_file)
            .unwrap()
            .file_type()
            .is_file()
    );
    assert_eq!(fs::read_to_string(&target_file).unwrap(), "# alpha");

    fs::write(&source_file, "# alpha updated").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("plan")
        .arg("copy-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("update"));

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("doctor")
        .arg("copy-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("managed_drift"));

    fs::write(&target_file, "edited target").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("undo")
        .assert()
        .failure()
        .stderr(predicates::str::contains("recorded post-apply state"));
}

#[test]
fn hardlink_mode_creates_linked_tree_and_doctor_detects_relation_drift() {
    use std::os::unix::fs::MetadataExt;

    let harness = Harness::new();
    let state_dir = harness.path().join("state");

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("hardlink-safe")
        .assert()
        .success();

    let source_file = harness.source_root().join("Skills/alpha/SKILL.md");
    let target_file = harness.dest_root().join("hardlinked-skills/alpha/SKILL.md");
    let source_meta = fs::metadata(&source_file).unwrap();
    let target_meta = fs::metadata(&target_file).unwrap();

    assert!(target_meta.is_file());
    assert_eq!(source_meta.ino(), target_meta.ino());
    assert_eq!(source_meta.dev(), target_meta.dev());

    fs::remove_file(&source_file).unwrap();
    fs::write(&source_file, "# alpha").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("doctor")
        .arg("hardlink-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("managed_drift"));

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("undo")
        .assert()
        .success()
        .stdout(predicates::str::contains("targets reverted"));

    assert!(!target_file.exists());
}

#[test]
fn prompted_profile_plan_blocks_when_required_composition_is_missing() {
    let harness = Harness::new();

    let output = bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("profile")
        .arg("plan")
        .arg("prompted")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("danger"));
    assert!(stdout.contains("required composition 'agent' is missing"));
    assert!(stdout.contains("build/prompts/AGENTS.generated.md"));
}

#[test]
fn prompted_profile_apply_blocks_when_required_composition_is_stale() {
    let harness = Harness::new();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("prompt")
        .arg("build")
        .arg("agent")
        .assert()
        .success();

    fs::write(harness.source_root().join("USER.md"), "user updated").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("profile")
        .arg("apply")
        .arg("prompted")
        .assert()
        .failure()
        .stdout(predicates::str::contains("required composition 'agent' is stale"))
        .stderr(predicates::str::contains("danger actions"));
}

#[test]
fn ordinary_apply_still_blocks_forceable_danger_collisions() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    harness.seed_takeover_collisions();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("takeover-safe")
        .assert()
        .failure()
        .stdout(predicates::str::contains("danger*"))
        .stderr(predicates::str::contains("danger actions"));
}

#[test]
fn force_with_backup_replaces_unmanaged_targets_and_undo_restores_them() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    harness.seed_takeover_collisions();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("takeover-safe")
        .arg("--force-with-backup")
        .assert()
        .success()
        .stdout(predicates::str::contains("danger*"));

    let takeover_root = harness.dest_root().join("takeover");
    assert_eq!(
        fs::read_to_string(takeover_root.join("file.md")).unwrap(),
        "notes"
    );
    assert!(
        fs::symlink_metadata(takeover_root.join("dir/alpha"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::symlink_metadata(takeover_root.join("link.md"))
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let backup_root = state_dir.join("backups");
    assert!(backup_root.exists());

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("undo")
        .assert()
        .success()
        .stdout(predicates::str::contains("targets reverted"));

    assert_eq!(
        fs::read_to_string(takeover_root.join("file.md")).unwrap(),
        "manual file"
    );
    assert_eq!(
        fs::read_to_string(takeover_root.join("dir/alpha/manual.txt")).unwrap(),
        "manual dir"
    );
    let restored_link = fs::read_link(takeover_root.join("link.md")).unwrap();
    assert_eq!(restored_link, PathBuf::from("manual-target.txt"));
    assert!(!backup_root.exists());
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
        fs::create_dir_all(source_root.join("Agents")).unwrap();
        fs::create_dir_all(source_root.join("Notes")).unwrap();
        fs::write(source_root.join("Skills/alpha/SKILL.md"), "# alpha").unwrap();
        fs::write(source_root.join("Skills/beta/SKILL.md"), "# beta").unwrap();
        fs::write(source_root.join("Agents/assistant.md"), "assistant").unwrap();
        fs::write(source_root.join("Notes/notes.md"), "notes").unwrap();
        fs::write(source_root.join("USER.md"), "user").unwrap();
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

    fn seed_takeover_collisions(&self) {
        let takeover_root = self.dest_root().join("takeover");
        fs::create_dir_all(&takeover_root).unwrap();
        fs::write(takeover_root.join("file.md"), "manual file").unwrap();
        fs::create_dir_all(takeover_root.join("dir/alpha")).unwrap();
        fs::write(takeover_root.join("dir/alpha/manual.txt"), "manual dir").unwrap();
        fs::write(takeover_root.join("manual-target.txt"), "manual target").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("manual-target.txt", takeover_root.join("link.md")).unwrap();
    }
}
