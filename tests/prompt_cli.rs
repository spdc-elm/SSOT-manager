use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("ssot-manager").expect("binary should build")
}

#[test]
fn prompt_list_and_show_report_configured_compositions() {
    let fixture = prepare_fixture("composition-valid.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::write(temp.path().join("source/USER.md"), "user").unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("agent"))
        .stdout(predicates::str::contains("inputs=2"));

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("prompt")
        .arg("show")
        .arg("agent")
        .assert()
        .success()
        .stdout(predicates::str::contains("Composition 'agent':"))
        .stdout(predicates::str::contains("renderer=concat"))
        .stdout(predicates::str::contains("path=Agents/assistant.md"))
        .stdout(predicates::str::contains("host=codex"));
}

#[test]
fn prompt_preview_and_build_render_wrapped_output() {
    let fixture = prepare_fixture("composition-valid.yaml", |temp| {
        fs::create_dir_all(temp.path().join("source/Agents")).unwrap();
        fs::write(temp.path().join("source/Agents/assistant.md"), "assistant").unwrap();
        fs::write(temp.path().join("source/USER.md"), "user").unwrap();
        fs::create_dir_all(temp.path().join("dest")).unwrap();
    });

    let preview = bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("prompt")
        .arg("preview")
        .arg("agent")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview = String::from_utf8(preview).unwrap();

    assert!(preview.contains("<prompt host=\"codex\" kind=\"system\">"));
    assert!(preview.contains("<assistant path=\"Agents/assistant.md\">"));
    assert!(preview.contains("<user path=\"USER.md\">"));
    assert!(
        !fixture
            .root()
            .join("source/build/prompts/AGENTS.generated.md")
            .exists()
    );

    bin()
        .arg("--config")
        .arg(fixture.config_path())
        .arg("prompt")
        .arg("build")
        .arg("agent")
        .assert()
        .success()
        .stdout(predicates::str::contains("Built composition 'agent'"));

    let built = fs::read_to_string(
        fixture
            .root()
            .join("source/build/prompts/AGENTS.generated.md"),
    )
    .unwrap();
    assert!(built.contains("<prompt host=\"codex\" kind=\"system\">"));
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
