use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use path_clean::PathClean;
use pathdiff::diff_paths;

pub fn expand_tilde(input: &str) -> Result<PathBuf> {
    if input == "~" {
        return home_dir();
    }

    if let Some(stripped) = input.strip_prefix("~/") {
        return Ok(home_dir()?.join(stripped));
    }

    Ok(PathBuf::from(input))
}

pub fn resolve_input_path(input: &str, base_dir: &Path) -> Result<PathBuf> {
    let path = expand_tilde(input)?;
    if path.is_absolute() {
        return Ok(path.clean());
    }

    Ok(base_dir.join(path).clean())
}

pub fn normalize(path: &Path) -> PathBuf {
    path.clean()
}

pub fn default_state_dir() -> Result<PathBuf> {
    if let Ok(dir) = env::var("XDG_STATE_HOME") {
        let path = PathBuf::from(dir);
        if path.is_absolute() {
            return Ok(path.join("ssot-manager").clean());
        }
    }

    Ok(home_dir()?.join(".local/state/ssot-manager").clean())
}

pub fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn resolved_link_target(link_path: &Path, raw_target: &Path) -> PathBuf {
    if raw_target.is_absolute() {
        return normalize(raw_target);
    }

    let parent = link_path.parent().unwrap_or_else(|| Path::new("."));
    normalize(&parent.join(raw_target))
}

pub fn symlink_target_for(source: &Path, target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    diff_paths(source, parent).unwrap_or_else(|| source.to_path_buf())
}

pub fn effective_target_path(target: &Path) -> Result<PathBuf> {
    let target = normalize(target);
    let leaf = target.file_name().map(|name| name.to_os_string());
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    let resolved_parent = resolve_existing_ancestor_symlinks(parent, 32)?;

    match leaf {
        Some(leaf) => Ok(normalize(&resolved_parent.join(leaf))),
        None => Ok(resolved_parent),
    }
}

fn resolve_existing_ancestor_symlinks(path: &Path, remaining_hops: usize) -> Result<PathBuf> {
    if remaining_hops == 0 {
        bail!(
            "failed to resolve ancestor symlinks for {}: too many nested symlinks",
            path.display()
        );
    }

    let path = normalize(path);
    let mut resolved = PathBuf::new();
    let mut missing_tail = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => resolved.push(component.as_os_str()),
            Component::Normal(part) => {
                resolved.push(part);
                if missing_tail {
                    continue;
                }

                match fs::symlink_metadata(&resolved) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        let raw_target = fs::read_link(&resolved).with_context(|| {
                            format!("failed to read link {}", resolved.display())
                        })?;
                        resolved = resolve_existing_ancestor_symlinks(
                            &resolved_link_target(&resolved, &raw_target),
                            remaining_hops - 1,
                        )?;
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        missing_tail = true;
                    }
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("failed to inspect {}", resolved.display()));
                    }
                }
            }
        }
    }

    Ok(normalize(&resolved))
}

fn home_dir() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME is not set")?;
    let path = PathBuf::from(home);
    if !path.is_absolute() {
        bail!("HOME must be an absolute path");
    }

    Ok(path.clean())
}
