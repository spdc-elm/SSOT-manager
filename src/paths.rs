use std::env;
use std::path::{Path, PathBuf};

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

fn home_dir() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME is not set")?;
    let path = PathBuf::from(home);
    if !path.is_absolute() {
        bail!("HOME must be an absolute path");
    }

    Ok(path.clean())
}
