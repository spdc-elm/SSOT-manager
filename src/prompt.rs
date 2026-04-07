use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

use crate::config::{Composition, Config, TemplateWrapper};
use crate::paths::path_to_string;

#[derive(Debug, Clone, Serialize)]
pub struct CompositionListView {
    pub compositions: Vec<CompositionSummaryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositionSummaryView {
    pub name: String,
    pub output: String,
    pub input_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositionShowView {
    pub composition_name: String,
    pub output: String,
    pub variables: BTreeMap<String, String>,
    pub renderer_kind: String,
    pub outer_wrapper: WrapperView,
    pub inputs: Vec<CompositionInputView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositionInputView {
    pub index: usize,
    pub path: String,
    pub wrapper: WrapperView,
}

#[derive(Debug, Clone, Serialize)]
pub struct WrapperView {
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositionRequirementView {
    pub name: String,
    pub output: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CompiledComposition {
    pub composition_name: String,
    pub output: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct BuildCompositionResult {
    pub composition_name: String,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionReadiness {
    Ready,
    Missing,
    Stale,
}

#[derive(Debug, Clone)]
pub struct CompositionStatus {
    pub name: String,
    pub output: PathBuf,
    pub readiness: CompositionReadiness,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct BuildProfileRequirementsResult {
    pub profile_name: String,
    pub built: Vec<BuildCompositionResult>,
}

pub fn list_compositions(config: &Config) -> CompositionListView {
    let compositions = config
        .compositions
        .values()
        .map(|composition| CompositionSummaryView {
            name: composition.name.clone(),
            output: path_to_string(&composition.output),
            input_count: composition.inputs.len(),
        })
        .collect();

    CompositionListView { compositions }
}

pub fn show_composition(config: &Config, composition_name: &str) -> Result<CompositionShowView> {
    let composition = composition(config, composition_name)?;

    Ok(CompositionShowView {
        composition_name: composition.name.clone(),
        output: path_to_string(&composition.output),
        variables: composition.variables.clone(),
        renderer_kind: composition.renderer.as_str().to_string(),
        outer_wrapper: wrapper_view(composition.renderer.outer_wrapper()),
        inputs: composition
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| CompositionInputView {
                index: index + 1,
                path: input.path.clone(),
                wrapper: wrapper_view(&input.wrapper),
            })
            .collect(),
    })
}

pub fn preview_composition(config: &Config, composition_name: &str) -> Result<String> {
    Ok(render_composition(config, composition_name)?.content)
}

pub fn build_composition(
    config: &Config,
    composition_name: &str,
) -> Result<BuildCompositionResult> {
    let compiled = render_composition(config, composition_name)?;
    if let Some(parent) = compiled.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&compiled.output, compiled.content.as_bytes())
        .with_context(|| format!("failed to write {}", compiled.output.display()))?;

    Ok(BuildCompositionResult {
        composition_name: compiled.composition_name,
        output: compiled.output,
    })
}

pub fn composition_status(config: &Config, composition_name: &str) -> Result<CompositionStatus> {
    let composition = composition(config, composition_name)?;

    let rendered = render_composition(config, composition_name)?;
    match fs::read(&composition.output) {
        Ok(current) => {
            if current == rendered.content.as_bytes() {
                Ok(CompositionStatus {
                    name: composition.name.clone(),
                    output: composition.output.clone(),
                    readiness: CompositionReadiness::Ready,
                    message: "generated output is up to date".to_string(),
                })
            } else {
                Ok(CompositionStatus {
                    name: composition.name.clone(),
                    output: composition.output.clone(),
                    readiness: CompositionReadiness::Stale,
                    message: "generated output is stale".to_string(),
                })
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CompositionStatus {
            name: composition.name.clone(),
            output: composition.output.clone(),
            readiness: CompositionReadiness::Missing,
            message: "generated output is missing".to_string(),
        }),
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect {}", composition.output.display())),
    }
}

pub fn profile_requirements(
    config: &Config,
    profile_name: &str,
) -> Result<Vec<CompositionRequirementView>> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow!("unknown profile '{profile_name}'"))?;

    profile
        .requires
        .iter()
        .map(|composition_name| {
            let status = composition_status(config, composition_name)?;
            Ok(CompositionRequirementView {
                name: status.name,
                output: path_to_string(&status.output),
                status: status.readiness.as_str().to_string(),
                message: status.message,
            })
        })
        .collect()
}

pub fn build_profile_requirements(
    config: &Config,
    profile_name: &str,
) -> Result<BuildProfileRequirementsResult> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow!("unknown profile '{profile_name}'"))?;
    let mut built = Vec::new();

    for composition_name in &profile.requires {
        built.push(build_composition(config, composition_name)?);
    }

    Ok(BuildProfileRequirementsResult {
        profile_name: profile_name.to_string(),
        built,
    })
}

pub fn render_composition(config: &Config, composition_name: &str) -> Result<CompiledComposition> {
    let composition = composition(config, composition_name)?;
    let mut rendered_inputs = String::new();

    for input in &composition.inputs {
        let content = fs::read_to_string(&input.resolved_path)
            .with_context(|| format!("failed to read {}", input.resolved_path.display()))?;
        let mut vars = composition.variables.clone();
        vars.insert("path".to_string(), input.path.clone());

        rendered_inputs.push_str(&render_template(&input.wrapper.before, &vars)?);
        rendered_inputs.push_str(&content);
        rendered_inputs.push_str(&render_template(&input.wrapper.after, &vars)?);
    }

    let outer_before = render_template(
        &composition.renderer.outer_wrapper().before,
        &composition.variables,
    )?;
    let outer_after = render_template(
        &composition.renderer.outer_wrapper().after,
        &composition.variables,
    )?;

    Ok(CompiledComposition {
        composition_name: composition.name.clone(),
        output: composition.output.clone(),
        content: format!("{outer_before}{rendered_inputs}{outer_after}"),
    })
}

fn composition<'a>(config: &'a Config, composition_name: &str) -> Result<&'a Composition> {
    config
        .compositions
        .get(composition_name)
        .ok_or_else(|| anyhow!("unknown composition '{composition_name}'"))
}

fn render_template(template: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    let mut remaining = template;
    let mut rendered = String::new();

    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let end = after_start
            .find("}}")
            .ok_or_else(|| anyhow!("unclosed placeholder in template"))?;
        let key = after_start[..end].trim();
        if key.is_empty() {
            bail!("empty placeholder in template");
        }
        let value = vars
            .get(key)
            .ok_or_else(|| anyhow!("undefined variable '{key}' in template"))?;
        rendered.push_str(value);
        remaining = &after_start[end + 2..];
    }

    rendered.push_str(remaining);
    Ok(rendered)
}

fn wrapper_view(wrapper: &TemplateWrapper) -> WrapperView {
    WrapperView {
        before: wrapper.before.clone(),
        after: wrapper.after.clone(),
    }
}

impl CompositionReadiness {
    pub fn as_str(self) -> &'static str {
        match self {
            CompositionReadiness::Ready => "ready",
            CompositionReadiness::Missing => "missing",
            CompositionReadiness::Stale => "stale",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::render_template;

    #[test]
    fn render_template_replaces_known_variables() {
        let mut vars = BTreeMap::new();
        vars.insert("host".to_string(), "codex".to_string());
        vars.insert("path".to_string(), "USER.md".to_string());

        let rendered = render_template("<x host=\"{{ host }}\">{{path}}</x>", &vars).unwrap();
        assert_eq!(rendered, "<x host=\"codex\">USER.md</x>");
    }

    #[test]
    fn render_template_rejects_unknown_variables() {
        let error = render_template("{{missing}}", &BTreeMap::new()).unwrap_err();
        assert!(error.to_string().contains("undefined variable"));
    }

    #[test]
    fn readiness_strings_are_stable() {
        assert_eq!(super::CompositionReadiness::Ready.as_str(), "ready");
        assert_eq!(super::CompositionReadiness::Missing.as_str(), "missing");
        assert_eq!(super::CompositionReadiness::Stale.as_str(), "stale");
    }

    #[test]
    fn build_result_is_constructible() {
        let result = super::BuildCompositionResult {
            composition_name: "agent".to_string(),
            output: PathBuf::from("/tmp/out"),
        };
        assert_eq!(result.composition_name, "agent");
    }
}
