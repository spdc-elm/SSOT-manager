use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::{MaterializationMode, SyncIntent};
use crate::paths::{default_state_dir, normalize, path_to_string, resolved_link_target};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManagedState {
    #[serde(default)]
    pub records: BTreeMap<String, ManagedRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedRecord {
    pub profile: String,
    pub source: String,
    pub target: String,
    pub mode: MaterializationMode,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyJournal {
    pub profile: String,
    pub applied_at: u64,
    pub entries: Vec<JournalEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub action: String,
    pub target: String,
    pub before: PathState,
    pub after: PathState,
    pub record_before: Option<ManagedRecord>,
    pub record_after: Option<ManagedRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PathState {
    Missing,
    Symlink { target: String },
    File,
    Directory,
    Other,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    root: PathBuf,
}

impl StateStore {
    pub fn new(root: Option<PathBuf>) -> Result<Self> {
        let root = match root {
            Some(path) => normalize(&path),
            None => default_state_dir()?,
        };

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load(&self) -> Result<ManagedState> {
        let path = self.managed_state_path();
        if !path.exists() {
            return Ok(ManagedState::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let state = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(state)
    }

    pub fn save(&self, state: &ManagedState) -> Result<()> {
        self.ensure_root()?;
        write_json(&self.managed_state_path(), state)
    }

    pub fn write_last_apply(&self, journal: &ApplyJournal) -> Result<()> {
        self.ensure_root()?;
        write_json(&self.last_apply_path(), journal)
    }

    pub fn load_last_apply(&self) -> Result<ApplyJournal> {
        let path = self.last_apply_path();
        if !path.exists() {
            bail!("no last apply journal found at {}", path.display());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let journal = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(journal)
    }

    pub fn clear_last_apply(&self) -> Result<()> {
        let path = self.last_apply_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        Ok(())
    }

    fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create {}", self.root.display()))
    }

    fn managed_state_path(&self) -> PathBuf {
        self.root.join("managed-records.json")
    }

    fn last_apply_path(&self) -> PathBuf {
        self.root.join("last-apply.json")
    }
}

pub fn snapshot_path(path: &Path) -> Result<PathState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                let target = fs::read_link(path)
                    .with_context(|| format!("failed to read link {}", path.display()))?;
                return Ok(PathState::Symlink {
                    target: path_to_string(&target),
                });
            }
            if file_type.is_dir() {
                return Ok(PathState::Directory);
            }
            if file_type.is_file() {
                return Ok(PathState::File);
            }
            Ok(PathState::Other)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PathState::Missing),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

pub fn build_record(intent: &SyncIntent, timestamp: u64) -> ManagedRecord {
    ManagedRecord {
        profile: intent.profile_name.clone(),
        source: path_to_string(&intent.source),
        target: path_to_string(&intent.target),
        mode: intent.mode,
        updated_at: timestamp,
    }
}

pub fn now_timestamp() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?;
    Ok(duration.as_secs())
}

pub fn restore_path(path: &Path, desired: &PathState) -> Result<()> {
    remove_existing_path(path)?;

    match desired {
        PathState::Missing => Ok(()),
        PathState::Symlink { target } => create_symlink(path, Path::new(target)),
        PathState::File | PathState::Directory | PathState::Other => {
            bail!(
                "undo cannot restore non-symlink content at {}",
                path.display()
            )
        }
    }
}

pub fn create_symlink(target: &Path, link_target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(link_target, target).with_context(|| {
            format!(
                "failed to create symlink {} -> {}",
                target.display(),
                link_target.display()
            )
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = link_target;
        bail!("symlink mode is only supported on unix platforms");
    }

    Ok(())
}

pub fn remove_existing_path(path: &Path) -> Result<()> {
    match snapshot_path(path)? {
        PathState::Missing => Ok(()),
        PathState::Directory => fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display())),
        PathState::Symlink { .. } | PathState::File | PathState::Other => {
            fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
        }
    }
}

pub fn symlink_matches_expected(path: &Path, current: &PathState, expected_source: &Path) -> bool {
    match current {
        PathState::Symlink { target } => {
            resolved_link_target(path, Path::new(target)) == normalize(expected_source)
        }
        _ => false,
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(path, payload).with_context(|| format!("failed to write {}", path.display()))
}
