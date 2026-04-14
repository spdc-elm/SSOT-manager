use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use globset::{Glob, GlobBuilder};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::paths::{normalize, path_to_string, resolve_input_path};

#[derive(Debug, Clone)]
pub struct Config {
    pub version: u64,
    pub config_path: PathBuf,
    pub source_root: PathBuf,
    pub config_dir: PathBuf,
    pub profiles: BTreeMap<String, Profile>,
    pub compositions: BTreeMap<String, Composition>,
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub source_root: PathBuf,
    pub rules: Vec<Rule>,
    pub requires: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub select: String,
    pub to: Vec<String>,
    pub mode: MaterializationMode,
    pub ignore: Vec<String>,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Composition {
    pub name: String,
    pub inputs: Vec<CompositionInput>,
    pub variables: BTreeMap<String, String>,
    pub renderer: PromptRenderer,
    pub output: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompositionInput {
    pub path: String,
    pub resolved_path: PathBuf,
    pub wrapper: TemplateWrapper,
}

#[derive(Debug, Clone)]
pub struct TemplateWrapper {
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone)]
pub enum PromptRenderer {
    Concat { outer_wrapper: TemplateWrapper },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializationMode {
    Symlink,
    Copy,
    Hardlink,
}

#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    pub profile_name: String,
    pub intents: Vec<SyncIntent>,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct SyncIntent {
    pub profile_name: String,
    pub source: PathBuf,
    pub target: PathBuf,
    pub mode: MaterializationMode,
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct EditableConfigDocument {
    pub path: PathBuf,
    pub config_dir: PathBuf,
    pub config: EditableConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableConfig {
    pub version: u64,
    pub source_root: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub compositions: BTreeMap<String, EditableComposition>,
    pub profiles: BTreeMap<String, EditableProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableProfile {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<EditableRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableRule {
    pub select: String,
    pub to: Vec<String>,
    pub mode: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
    #[serde(default = "default_true")]
    #[serde(skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableComposition {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<EditableCompositionInput>,
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
    pub renderer: EditableRenderer,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableCompositionInput {
    pub path: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrapper: Option<EditableTemplateWrapper>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableTemplateWrapper {
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditableRenderer {
    pub kind: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outer_wrapper: Option<EditableTemplateWrapper>,
}

pub fn load_config(config_path: &Path) -> Result<Config> {
    let document = load_editable_config(config_path)?;
    validate_editable_config(&document)
}

pub fn load_editable_config(config_path: &Path) -> Result<EditableConfigDocument> {
    let config_path = resolve_config_path(config_path)?;
    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config {}", config_path.display()))?;
    let config: EditableConfig = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse YAML {}", config_path.display()))?;
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(EditableConfigDocument {
        path: config_path,
        config_dir: normalize(&config_dir),
        config,
    })
}

pub fn validate_editable_config(document: &EditableConfigDocument) -> Result<Config> {
    validate_editable_config_at(&document.config, &document.path)
}

pub fn validate_editable_config_at(config: &EditableConfig, config_path: &Path) -> Result<Config> {
    let config_path = resolve_config_path(config_path)?;
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    validate_editable_config_model(config, &config_path, &config_dir)
}

pub fn write_editable_config(document: &EditableConfigDocument) -> Result<()> {
    let contents = serde_yaml::to_string(&document.config)
        .context("failed to serialize editable config to YAML")?;
    atomic_write_text(&document.path, &contents)
}

pub fn validate_and_write_editable_config(document: &EditableConfigDocument) -> Result<Config> {
    let config = validate_editable_config(document)?;
    write_editable_config(document)?;
    Ok(config)
}

pub fn resolve_profile(config: &Config, profile_name: &str) -> Result<ResolvedProfile> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow!("unknown profile '{profile_name}'"))?;

    let mut intents = Vec::new();
    let mut diagnostics = Vec::new();
    let mut targets = HashMap::<String, String>::new();

    for rule in profile.rules.iter().filter(|rule| rule.enabled) {
        let matches = discover_assets(&profile.source_root, &rule.select)?;
        if matches.is_empty() {
            diagnostics.push(ConfigDiagnostic {
                code: "asset_not_found",
                message: format!(
                    "rule select '{}' matched no assets under {}",
                    rule.select,
                    profile.source_root.display()
                ),
            });
            continue;
        }

        for destination in &rule.to {
            let destination_root = resolve_input_path(destination, &config.config_dir)?;
            for source in &matches {
                let target =
                    resolve_target_path(destination, &destination_root, source, matches.len())?;
                let target_key = path_to_string(&target);
                let source_key = path_to_string(source);

                if let Some(existing) = targets.get(&target_key) {
                    if existing != &source_key {
                        bail!(
                            "target '{}' resolves from multiple sources: '{}' and '{}'",
                            target.display(),
                            existing,
                            source.display()
                        );
                    }
                    continue;
                }

                targets.insert(target_key, source_key);
                intents.push(SyncIntent {
                    profile_name: profile_name.to_string(),
                    source: source.clone(),
                    target,
                    mode: rule.mode,
                    ignore: rule.ignore.clone(),
                });
            }
        }
    }

    Ok(ResolvedProfile {
        profile_name: profile_name.to_string(),
        intents,
        diagnostics,
    })
}

fn validate_editable_config_model(
    raw: &EditableConfig,
    config_path: &Path,
    config_dir: &Path,
) -> Result<Config> {
    if raw.version != 1 {
        bail!("unsupported config version '{}'; expected 1", raw.version);
    }

    let source_root = resolve_input_path(&raw.source_root, &config_dir)?;
    validate_source_root(&source_root, "source_root")?;
    let source_root = normalize(&source_root);

    let compositions = validate_compositions(&raw.compositions, &source_root)?;
    let mut profiles = BTreeMap::new();

    for (name, profile) in &raw.profiles {
        if name.trim().is_empty() {
            bail!("profile names must not be empty");
        }

        let profile_source_root = match profile.source_root {
            Some(ref path) => {
                let path = resolve_input_path(&path, &config_dir)?;
                validate_source_root(&path, &format!("profile '{name}' source_root"))?;
                normalize(&path)
            }
            None => source_root.clone(),
        };

        for required in &profile.requires {
            if !compositions.contains_key(required) {
                bail!(
                    "profile '{name}' requires undefined composition '{}'",
                    required
                );
            }
        }

        let mut rules = Vec::new();
        for (index, rule) in profile.rules.iter().enumerate() {
            if rule.select.trim().is_empty() {
                bail!("profile '{name}' rule {} has an empty select", index + 1);
            }
            if rule.to.is_empty() {
                bail!("profile '{name}' rule {} has no destinations", index + 1);
            }
            Glob::new(&rule.select).with_context(|| {
                format!(
                    "profile '{name}' rule {} has invalid select glob",
                    index + 1
                )
            })?;
            GlobBuilder::new(&rule.select)
                .literal_separator(true)
                .build()
                .with_context(|| {
                    format!(
                        "profile '{name}' rule {} has invalid select glob",
                        index + 1
                    )
                })?;

            let mode = parse_materialization_mode(&rule.mode).with_context(|| {
                format!("profile '{name}' rule {} uses invalid mode", index + 1)
            })?;
            for pattern in &rule.ignore {
                validate_glob_pattern(pattern).with_context(|| {
                    format!("profile '{name}' rule {} has invalid ignore glob", index + 1)
                })?;
            }

            for destination in &rule.to {
                if destination.trim().is_empty() {
                    bail!(
                        "profile '{name}' rule {} contains an empty destination",
                        index + 1
                    );
                }
            }

            rules.push(Rule {
                select: rule.select.clone(),
                to: rule.to.clone(),
                mode,
                ignore: rule.ignore.clone(),
                enabled: rule.enabled,
                tags: rule.tags.clone(),
                note: rule.note.clone(),
            });
        }

        profiles.insert(
            name.clone(),
            Profile {
                source_root: profile_source_root,
                rules,
                requires: profile.requires.clone(),
            },
        );
    }

    Ok(Config {
        version: raw.version,
        config_path: normalize(config_path),
        source_root,
        config_dir: normalize(config_dir),
        profiles,
        compositions,
    })
}

fn validate_compositions(
    raw: &BTreeMap<String, EditableComposition>,
    source_root: &Path,
) -> Result<BTreeMap<String, Composition>> {
    let mut compositions = BTreeMap::new();

    for (name, composition) in raw {
        if composition.inputs.is_empty() {
            bail!("composition '{name}' has no inputs");
        }
        if composition.output.trim().is_empty() {
            bail!("composition '{name}' has an empty output");
        }

        let output = resolve_input_path(&composition.output, source_root)?;
        ensure_path_within_root(
            &output,
            source_root,
            &format!("composition '{name}' output"),
        )?;

        let inputs = composition
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| validate_composition_input(name, index, input, source_root))
            .collect::<Result<Vec<_>>>()?;

        let renderer = validate_renderer(name, &composition.renderer)?;

        compositions.insert(
            name.clone(),
            Composition {
                name: name.clone(),
                inputs,
                variables: composition.variables.clone(),
                renderer,
                output: normalize(&output),
            },
        );
    }

    Ok(compositions)
}

fn validate_composition_input(
    composition_name: &str,
    index: usize,
    input: &EditableCompositionInput,
    source_root: &Path,
) -> Result<CompositionInput> {
    if input.path.trim().is_empty() {
        bail!(
            "composition '{}' input {} has an empty path",
            composition_name,
            index + 1
        );
    }

    let resolved_path = resolve_input_path(&input.path, source_root)?;
    ensure_path_within_root(
        &resolved_path,
        source_root,
        &format!("composition '{}' input {}", composition_name, index + 1),
    )?;

    Ok(CompositionInput {
        path: input.path.clone(),
        resolved_path: normalize(&resolved_path),
        wrapper: normalize_wrapper(input.wrapper.clone()),
    })
}

fn validate_renderer(
    composition_name: &str,
    renderer: &EditableRenderer,
) -> Result<PromptRenderer> {
    match renderer.kind.as_str() {
        "concat" => Ok(PromptRenderer::Concat {
            outer_wrapper: normalize_wrapper(renderer.outer_wrapper.clone()),
        }),
        "script" => bail!(
            "composition '{}' uses unsupported renderer 'script'; scripted renderers are not yet supported",
            composition_name
        ),
        other => bail!(
            "composition '{}' uses unknown renderer '{}'",
            composition_name,
            other
        ),
    }
}

fn normalize_wrapper(raw: Option<EditableTemplateWrapper>) -> TemplateWrapper {
    let raw = raw.unwrap_or(EditableTemplateWrapper {
        before: String::new(),
        after: String::new(),
    });

    TemplateWrapper {
        before: raw.before,
        after: raw.after,
    }
}

fn parse_materialization_mode(raw: &str) -> Result<MaterializationMode> {
    match raw {
        "symlink" => Ok(MaterializationMode::Symlink),
        "copy" => Ok(MaterializationMode::Copy),
        "hardlink" => Ok(MaterializationMode::Hardlink),
        other => bail!("uses unknown mode '{other}'"),
    }
}

fn validate_glob_pattern(pattern: &str) -> Result<()> {
    Glob::new(pattern)?;
    GlobBuilder::new(pattern).literal_separator(true).build()?;
    Ok(())
}

fn ensure_path_within_root(path: &Path, root: &Path, label: &str) -> Result<()> {
    let path = normalize(path);
    let root = normalize(root);
    if !path.starts_with(&root) {
        bail!(
            "{label} '{}' must stay under {}",
            path.display(),
            root.display()
        );
    }

    Ok(())
}

fn validate_source_root(source_root: &Path, label: &str) -> Result<()> {
    if !source_root.is_absolute() {
        bail!("{label} must be an absolute path");
    }
    if !source_root.exists() {
        bail!("{label} '{}' does not exist", source_root.display());
    }
    if !source_root.is_dir() {
        bail!("{label} '{}' must be a directory", source_root.display());
    }

    Ok(())
}

fn discover_assets(source_root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let matcher = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .with_context(|| format!("invalid select glob '{pattern}'"))?
        .compile_matcher();
    let mut matches = Vec::new();

    for entry in WalkDir::new(source_root).min_depth(1).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(source_root)
            .with_context(|| format!("failed to relativize {}", path.display()))?;
        let relative_string = relative.to_string_lossy().replace('\\', "/");
        if matcher.is_match(relative_string) {
            matches.push(normalize(path));
        }
    }

    matches.sort_by(|left, right| path_to_string(left).cmp(&path_to_string(right)));
    Ok(matches)
}

fn resolve_target_path(
    raw_destination: &str,
    resolved_destination: &Path,
    source: &Path,
    matched_count: usize,
) -> Result<PathBuf> {
    let treat_as_directory =
        raw_destination.ends_with('/') || resolved_destination.is_dir() || matched_count > 1;
    let source_name = source
        .file_name()
        .ok_or_else(|| anyhow!("source '{}' has no basename", source.display()))?;

    if matched_count > 1 && !raw_destination.ends_with('/') && !resolved_destination.is_dir() {
        bail!(
            "destination '{}' must end with '/' or exist as a directory when a rule matches multiple assets",
            raw_destination
        );
    }

    if treat_as_directory {
        return Ok(normalize(&resolved_destination.join(source_name)));
    }

    Ok(normalize(resolved_destination))
}

fn resolve_config_path(config_path: &Path) -> Result<PathBuf> {
    let path = if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(config_path)
    };

    Ok(normalize(&path))
}

fn atomic_write_text(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("config path '{}' has no parent", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("config path '{}' has no file name", path.display()))?
        .to_string_lossy()
        .into_owned();
    let tmp_path = parent.join(format!("{file_name}.tmp"));

    {
        let mut file = fs::File::create(&tmp_path)
            .with_context(|| format!("failed to create {}", tmp_path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write {}", tmp_path.display()))?;
        file.flush()
            .with_context(|| format!("failed to flush {}", tmp_path.display()))?;
    }

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to atomically replace {} with {}",
            path.display(),
            tmp_path.display()
        )
    })?;

    Ok(())
}

fn is_true(value: &bool) -> bool {
    *value
}

fn default_true() -> bool {
    true
}

impl MaterializationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            MaterializationMode::Symlink => "symlink",
            MaterializationMode::Copy => "copy",
            MaterializationMode::Hardlink => "hardlink",
        }
    }
}

impl PromptRenderer {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptRenderer::Concat { .. } => "concat",
        }
    }

    pub fn outer_wrapper(&self) -> &TemplateWrapper {
        match self {
            PromptRenderer::Concat { outer_wrapper } => outer_wrapper,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn editable_config_round_trips_to_normalized_yaml() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("config.yaml");
        fs::create_dir_all(temp.path().join("source/Skills/alpha")).unwrap();
        fs::write(temp.path().join("source/Skills/alpha/SKILL.md"), "# alpha").unwrap();

        let document = EditableConfigDocument {
            path: config_path.clone(),
            config_dir: temp.path().to_path_buf(),
            config: EditableConfig {
                version: 1,
                source_root: temp.path().join("source").display().to_string(),
                compositions: BTreeMap::new(),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    EditableProfile {
                        source_root: None,
                        requires: Vec::new(),
                        rules: vec![EditableRule {
                            select: "Skills/*".to_string(),
                            to: vec!["/tmp/example/skills/".to_string()],
                            mode: "symlink".to_string(),
                            ignore: Vec::new(),
                            enabled: true,
                            tags: Vec::new(),
                            note: None,
                        }],
                    },
                )]),
            },
        };

        write_editable_config(&document).unwrap();
        let contents = fs::read_to_string(&config_path).unwrap();

        assert!(contents.contains("version: 1"));
        assert!(contents.contains("source_root:"));
        assert!(contents.contains("profiles:"));
        assert!(contents.contains("main:"));
        assert!(contents.contains("mode: symlink"));
        assert!(!contents.contains("enabled: true"));
    }

    #[test]
    fn validate_editable_config_reuses_existing_validation_rules() {
        let temp = TempDir::new().unwrap();
        let source_root = temp.path().join("source");
        fs::create_dir_all(source_root.join("Skills/alpha")).unwrap();

        let editable = EditableConfig {
            version: 1,
            source_root: source_root.display().to_string(),
            compositions: BTreeMap::new(),
            profiles: BTreeMap::from([(
                "main".to_string(),
                EditableProfile {
                    source_root: None,
                    requires: Vec::new(),
                    rules: vec![EditableRule {
                        select: "Skills/*".to_string(),
                        to: vec!["/tmp/example".to_string()],
                        mode: "broken".to_string(),
                        ignore: Vec::new(),
                        enabled: true,
                        tags: Vec::new(),
                        note: None,
                    }],
                },
            )]),
        };

        let error =
            validate_editable_config_at(&editable, &temp.path().join("config.yaml")).unwrap_err();
        assert!(error.to_string().contains("uses invalid mode"));
    }

    #[test]
    fn validate_and_write_does_not_replace_file_on_validation_failure() {
        let temp = TempDir::new().unwrap();
        let source_root = temp.path().join("source");
        fs::create_dir_all(source_root.join("Skills/alpha")).unwrap();
        fs::write(source_root.join("Skills/alpha/SKILL.md"), "# alpha").unwrap();

        let config_path = temp.path().join("config.yaml");
        fs::write(
            &config_path,
            format!(
                "version: 1\nsource_root: {}\nprofiles:\n  main:\n    rules:\n      - select: Skills/*\n        to:\n          - /tmp/example/\n        mode: symlink\n",
                source_root.display()
            ),
        )
        .unwrap();

        let mut document = load_editable_config(&config_path).unwrap();
        let original = fs::read_to_string(&config_path).unwrap();
        document.config.profiles.get_mut("main").unwrap().rules[0].mode = "broken".to_string();

        let error = validate_and_write_editable_config(&document).unwrap_err();
        assert!(error.to_string().contains("uses invalid mode"));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
    }
}
