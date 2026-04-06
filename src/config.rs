use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use globset::{Glob, GlobBuilder};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::paths::{normalize, path_to_string, resolve_input_path};

#[derive(Debug, Clone)]
pub struct Config {
    pub version: u64,
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
}

#[derive(Debug, Clone)]
pub struct ConfigDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    version: u64,
    source_root: String,
    #[serde(default)]
    compositions: BTreeMap<String, RawComposition>,
    profiles: BTreeMap<String, RawProfile>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    source_root: Option<String>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    select: String,
    to: Vec<String>,
    mode: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    tags: Vec<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawComposition {
    #[serde(default)]
    inputs: Vec<RawCompositionInput>,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    renderer: RawRenderer,
    output: String,
}

#[derive(Debug, Deserialize)]
struct RawCompositionInput {
    path: String,
    wrapper: Option<RawTemplateWrapper>,
}

#[derive(Debug, Deserialize)]
struct RawTemplateWrapper {
    #[serde(default)]
    before: String,
    #[serde(default)]
    after: String,
}

#[derive(Debug, Deserialize)]
struct RawRenderer {
    kind: String,
    outer_wrapper: Option<RawTemplateWrapper>,
}

pub fn load_config(config_path: &Path) -> Result<Config> {
    let config_path = if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(config_path)
    };

    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config {}", config_path.display()))?;
    let raw: RawConfig = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse YAML {}", config_path.display()))?;
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    validate_raw_config(raw, config_dir)
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

fn validate_raw_config(raw: RawConfig, config_dir: PathBuf) -> Result<Config> {
    if raw.version != 1 {
        bail!("unsupported config version '{}'; expected 1", raw.version);
    }

    let source_root = resolve_input_path(&raw.source_root, &config_dir)?;
    validate_source_root(&source_root, "source_root")?;
    let source_root = normalize(&source_root);

    let compositions = validate_compositions(raw.compositions, &source_root)?;
    let mut profiles = BTreeMap::new();

    for (name, profile) in raw.profiles {
        let profile_source_root = match profile.source_root {
            Some(path) => {
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
        for (index, rule) in profile.rules.into_iter().enumerate() {
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

            for destination in &rule.to {
                if destination.trim().is_empty() {
                    bail!(
                        "profile '{name}' rule {} contains an empty destination",
                        index + 1
                    );
                }
            }

            rules.push(Rule {
                select: rule.select,
                to: rule.to,
                mode,
                enabled: rule.enabled,
                tags: rule.tags,
                note: rule.note,
            });
        }

        profiles.insert(
            name,
            Profile {
                source_root: profile_source_root,
                rules,
                requires: profile.requires,
            },
        );
    }

    Ok(Config {
        version: 1,
        source_root,
        config_dir: normalize(&config_dir),
        profiles,
        compositions,
    })
}

fn validate_compositions(
    raw: BTreeMap<String, RawComposition>,
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
            .into_iter()
            .enumerate()
            .map(|(index, input)| validate_composition_input(&name, index, input, source_root))
            .collect::<Result<Vec<_>>>()?;

        let renderer = validate_renderer(&name, composition.renderer)?;

        compositions.insert(
            name.clone(),
            Composition {
                name,
                inputs,
                variables: composition.variables,
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
    input: RawCompositionInput,
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
        path: input.path,
        resolved_path: normalize(&resolved_path),
        wrapper: normalize_wrapper(input.wrapper),
    })
}

fn validate_renderer(composition_name: &str, renderer: RawRenderer) -> Result<PromptRenderer> {
    match renderer.kind.as_str() {
        "concat" => Ok(PromptRenderer::Concat {
            outer_wrapper: normalize_wrapper(renderer.outer_wrapper),
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

fn normalize_wrapper(raw: Option<RawTemplateWrapper>) -> TemplateWrapper {
    let raw = raw.unwrap_or(RawTemplateWrapper {
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
