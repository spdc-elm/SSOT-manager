use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("ssot-manager").expect("binary should build")
}

fn apply_hardlink_safe(harness: &Harness, state_dir: &Path) {
    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(state_dir)
        .arg("profile")
        .arg("apply")
        .arg("hardlink-safe")
        .assert()
        .success();
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
fn self_referential_parent_symlink_plans_as_non_forceable_danger() {
    let harness = Harness::new();
    harness.seed_self_referential_parent_symlink();

    let output = bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("profile")
        .arg("plan")
        .arg("self-ref-parent-symlink")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("danger"));
    assert!(!stdout.contains("danger*"));
    assert!(stdout.contains("self-ref-parent/agents/assistant.md"));
    assert!(stdout.contains("overlaps managed source"));
}

#[test]
fn self_referential_parent_symlink_apply_refuses_before_mutation() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    harness.seed_self_referential_parent_symlink();

    let source_file = harness.source_root().join("Agents/assistant.md");
    let original_source = fs::read_to_string(&source_file).unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("self-ref-parent-symlink")
        .assert()
        .failure()
        .stdout(predicates::str::contains("overlaps managed source"))
        .stderr(predicates::str::contains("danger actions"));

    assert_eq!(fs::read_to_string(&source_file).unwrap(), original_source);

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("apply")
        .arg("self-ref-parent-symlink")
        .arg("--force-with-backup")
        .assert()
        .failure()
        .stdout(predicates::str::contains("overlaps managed source"))
        .stderr(predicates::str::contains("non-forceable danger actions"));

    assert_eq!(fs::read_to_string(&source_file).unwrap(), original_source);
}

#[test]
fn ordinary_matching_leaf_symlink_without_ancestor_overlap_still_skips() {
    let harness = Harness::new();
    fs::create_dir_all(harness.dest_root().join("safe-skills")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        harness.source_root().join("Skills/alpha"),
        harness.dest_root().join("safe-skills/alpha"),
    )
    .unwrap();

    let output = bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("profile")
        .arg("plan")
        .arg("skill-safe")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("safe-skills/alpha"));
    assert!(stdout.contains("target already matches the desired materialization"));
    assert!(stdout.contains("skip=1"));
    assert!(stdout.contains("danger=0"));
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
fn copy_mode_ignore_globs_skip_metadata_and_allow_undo() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    let source_ignored = harness.source_root().join("Skills/alpha/.DS_Store");
    fs::write(&source_ignored, "source-metadata").unwrap();

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

    let target_root = harness.dest_root().join("copied-skills/alpha");
    assert!(target_root.join("SKILL.md").exists());
    assert!(!target_root.join(".DS_Store").exists());

    let journal: Value =
        serde_json::from_str(&fs::read_to_string(state_dir.join("last-apply.json")).unwrap())
            .unwrap();
    assert_eq!(
        journal["entries"][0]["record_after"]["ignore"][0],
        "**/.DS_Store"
    );
    assert_eq!(
        journal["entries"][0]["record_after"]["ignore"][1],
        "**/Thumbs.db"
    );

    fs::write(target_root.join(".DS_Store"), "target-metadata").unwrap();

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
        .stdout(predicates::str::contains("skip"))
        .stdout(predicates::str::contains(
            "Summary: create=0 update=0 remove=0 skip=1",
        ))
        .stdout(predicates::str::contains(
            "target already matches the desired materialization",
        ));

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
        .stdout(predicates::str::contains("Doctor OK"));

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("undo")
        .assert()
        .success()
        .stdout(predicates::str::contains("targets reverted"));

    assert!(!target_root.exists());
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
fn hardlink_mode_ignore_globs_skip_metadata_in_plan_and_doctor() {
    use std::os::unix::fs::MetadataExt;

    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    let source_ignored = harness.source_root().join("Skills/alpha/.DS_Store");
    fs::write(&source_ignored, "source-metadata").unwrap();

    apply_hardlink_safe(&harness, &state_dir);

    let source_file = harness.source_root().join("Skills/alpha/SKILL.md");
    let target_root = harness.dest_root().join("hardlinked-skills/alpha");
    let target_file = target_root.join("SKILL.md");
    let source_meta = fs::metadata(&source_file).unwrap();
    let target_meta = fs::metadata(&target_file).unwrap();
    assert_eq!(source_meta.ino(), target_meta.ino());
    assert_eq!(source_meta.dev(), target_meta.dev());
    assert!(!target_root.join(".DS_Store").exists());

    fs::write(target_root.join(".DS_Store"), "target-metadata").unwrap();

    bin()
        .arg("--config")
        .arg(harness.config_path())
        .arg("--state-dir")
        .arg(&state_dir)
        .arg("profile")
        .arg("plan")
        .arg("hardlink-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("skip"))
        .stdout(predicates::str::contains(
            "Summary: create=0 update=0 remove=0 skip=1",
        ))
        .stdout(predicates::str::contains(
            "target already matches the desired materialization",
        ));

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
        .stdout(predicates::str::contains("Doctor OK"));
}

#[test]
fn hardlink_mode_updates_when_source_directory_gains_a_file() {
    use std::os::unix::fs::MetadataExt;

    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    apply_hardlink_safe(&harness, &state_dir);

    let source_file = harness.source_root().join("Skills/alpha/new-src.txt");
    let target_file = harness
        .dest_root()
        .join("hardlinked-skills/alpha/new-src.txt");
    fs::write(&source_file, "src-new").unwrap();

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
        .arg("profile")
        .arg("apply")
        .arg("hardlink-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("update"));

    let source_meta = fs::metadata(&source_file).unwrap();
    let target_meta = fs::metadata(&target_file).unwrap();
    assert!(target_meta.is_file());
    assert_eq!(source_meta.ino(), target_meta.ino());
    assert_eq!(source_meta.dev(), target_meta.dev());
}

#[test]
fn hardlink_mode_updates_when_source_directory_loses_a_file() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    apply_hardlink_safe(&harness, &state_dir);

    let source_file = harness.source_root().join("Skills/alpha/notes.txt");
    let target_file = harness
        .dest_root()
        .join("hardlinked-skills/alpha/notes.txt");
    fs::write(&source_file, "notes").unwrap();

    apply_hardlink_safe(&harness, &state_dir);
    fs::remove_file(&source_file).unwrap();

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
        .arg("profile")
        .arg("apply")
        .arg("hardlink-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("update"));

    assert!(!source_file.exists());
    assert!(!target_file.exists());
}

#[test]
fn hardlink_mode_updates_when_target_directory_gains_an_extra_file() {
    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    apply_hardlink_safe(&harness, &state_dir);

    let extra_target = harness
        .dest_root()
        .join("hardlinked-skills/alpha/extra-dst.txt");
    fs::write(&extra_target, "dst-extra").unwrap();

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
        .arg("profile")
        .arg("apply")
        .arg("hardlink-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("update"));

    assert!(!extra_target.exists());
}

#[test]
fn hardlink_mode_updates_when_target_directory_loses_a_file() {
    use std::os::unix::fs::MetadataExt;

    let harness = Harness::new();
    let state_dir = harness.path().join("state");
    apply_hardlink_safe(&harness, &state_dir);

    let source_file = harness.source_root().join("Skills/alpha/notes.txt");
    let target_file = harness
        .dest_root()
        .join("hardlinked-skills/alpha/notes.txt");
    fs::write(&source_file, "notes").unwrap();

    apply_hardlink_safe(&harness, &state_dir);
    fs::remove_file(&target_file).unwrap();

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
        .arg("profile")
        .arg("apply")
        .arg("hardlink-safe")
        .assert()
        .success()
        .stdout(predicates::str::contains("update"));

    let source_meta = fs::metadata(&source_file).unwrap();
    let target_meta = fs::metadata(&target_file).unwrap();
    assert!(target_meta.is_file());
    assert_eq!(source_meta.ino(), target_meta.ino());
    assert_eq!(source_meta.dev(), target_meta.dev());
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
        .stdout(predicates::str::contains(
            "required composition 'agent' is stale",
        ))
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

#[test]
fn force_with_backup_does_not_create_live_backup_artifacts_for_symlink_collisions() {
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
        .success();

    let journal: Value =
        serde_json::from_str(&fs::read_to_string(state_dir.join("last-apply.json")).unwrap())
            .unwrap();
    let entries = journal["entries"].as_array().unwrap();
    let link_entry = entries
        .iter()
        .find(|entry| {
            entry["target"]
                .as_str()
                .unwrap()
                .ends_with("/takeover/link.md")
        })
        .unwrap();

    assert!(link_entry["before"]["kind"] == "symlink");
    assert!(link_entry["backup_before"].is_null());
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

    fn seed_self_referential_parent_symlink(&self) {
        let self_ref_root = self.dest_root().join("self-ref-parent");
        fs::create_dir_all(&self_ref_root).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            self.source_root().join("Agents"),
            self_ref_root.join("agents"),
        )
        .unwrap();
    }
}
