use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{MaterializationMode, SyncIntent};
use crate::paths::{
    default_state_dir, normalize, path_to_string, resolved_link_target, symlink_target_for,
};

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
    #[serde(default)]
    pub record_before_source_state: Option<PathState>,
    #[serde(default)]
    pub backup_before: Option<BackupArtifact>,
    pub record_after: Option<ManagedRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupArtifact {
    pub path: String,
    pub state: PathState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PathState {
    Missing,
    Symlink {
        target: String,
        resolved_target: String,
    },
    File {
        size: u64,
        sha256: String,
        device: Option<u64>,
        inode: Option<u64>,
    },
    Directory {
        entries: BTreeMap<String, PathState>,
    },
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
        self.remove_backup_artifacts_for_current_journal()?;
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
        self.remove_backup_artifacts_for_current_journal()?;
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

    fn backups_root(&self) -> PathBuf {
        self.root.join("backups")
    }

    fn remove_backup_artifacts_for_current_journal(&self) -> Result<()> {
        let path = self.last_apply_path();
        if !path.exists() {
            return Ok(());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let journal: ApplyJournal = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;

        for entry in journal.entries {
            if let Some(backup) = entry.backup_before {
                remove_existing_path(Path::new(&backup.path))?;
            }
        }

        self.remove_empty_backup_dirs()?;

        Ok(())
    }

    fn remove_empty_backup_dirs(&self) -> Result<()> {
        let backups_root = self.backups_root();
        if !backups_root.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&backups_root)
            .with_context(|| format!("failed to read {}", backups_root.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }

        if fs::read_dir(&backups_root)?.next().is_none() {
            fs::remove_dir(&backups_root)
                .with_context(|| format!("failed to remove {}", backups_root.display()))?;
        }

        Ok(())
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
                    resolved_target: path_to_string(&resolved_link_target(path, &target)),
                });
            }
            if file_type.is_dir() {
                let mut entries = BTreeMap::new();
                for entry in fs::read_dir(path)
                    .with_context(|| format!("failed to read directory {}", path.display()))?
                {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    entries.insert(name, snapshot_path(&entry.path())?);
                }
                return Ok(PathState::Directory { entries });
            }
            if file_type.is_file() {
                return snapshot_file(path, &metadata);
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
        PathState::Symlink { target, .. } => create_symlink(path, Path::new(target)),
        PathState::File { .. } | PathState::Directory { .. } | PathState::Other => {
            bail!("undo cannot restore concrete content at {}", path.display())
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

pub fn materialize_target(source: &Path, target: &Path, mode: MaterializationMode) -> Result<()> {
    match mode {
        MaterializationMode::Symlink => {
            let link_target = symlink_target_for(source, target);
            create_symlink(target, &link_target)
        }
        MaterializationMode::Copy => materialize_copy(source, target),
        MaterializationMode::Hardlink => materialize_hardlink(source, target),
    }
}

pub fn remove_existing_path(path: &Path) -> Result<()> {
    match snapshot_path(path)? {
        PathState::Missing => Ok(()),
        PathState::Directory { .. } => fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display())),
        PathState::Symlink { .. } | PathState::File { .. } | PathState::Other => {
            fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
        }
    }
}

pub fn symlink_matches_expected(_path: &Path, current: &PathState, expected_source: &Path) -> bool {
    match current {
        PathState::Symlink {
            resolved_target, ..
        } => PathBuf::from(resolved_target) == normalize(expected_source),
        _ => false,
    }
}

pub fn path_state_content_matches(left: &PathState, right: &PathState) -> bool {
    match (left, right) {
        (PathState::Missing, PathState::Missing) => true,
        (
            PathState::Symlink {
                resolved_target: left_resolved,
                ..
            },
            PathState::Symlink {
                resolved_target: right_resolved,
                ..
            },
        ) => left_resolved == right_resolved,
        (
            PathState::File {
                size: left_size,
                sha256: left_sha256,
                ..
            },
            PathState::File {
                size: right_size,
                sha256: right_sha256,
                ..
            },
        ) => left_size == right_size && left_sha256 == right_sha256,
        (
            PathState::Directory {
                entries: left_entries,
            },
            PathState::Directory {
                entries: right_entries,
            },
        ) => {
            left_entries.len() == right_entries.len()
                && left_entries.iter().all(|(name, left_state)| {
                    right_entries.get(name).is_some_and(|right_state| {
                        path_state_content_matches(left_state, right_state)
                    })
                })
        }
        (PathState::Other, PathState::Other) => true,
        _ => false,
    }
}

pub fn path_state_hardlink_matches(left: &PathState, right: &PathState) -> bool {
    match (left, right) {
        (PathState::Missing, PathState::Missing) => true,
        (
            PathState::Symlink {
                resolved_target: left_resolved,
                ..
            },
            PathState::Symlink {
                resolved_target: right_resolved,
                ..
            },
        ) => left_resolved == right_resolved,
        (
            PathState::File {
                size: left_size,
                sha256: left_sha256,
                device: left_device,
                inode: left_inode,
            },
            PathState::File {
                size: right_size,
                sha256: right_sha256,
                device: right_device,
                inode: right_inode,
            },
        ) => {
            left_size == right_size
                && left_sha256 == right_sha256
                && left_device == right_device
                && left_inode == right_inode
        }
        (
            PathState::Directory {
                entries: left_entries,
            },
            PathState::Directory {
                entries: right_entries,
            },
        ) => {
            left_entries.len() == right_entries.len()
                && left_entries.iter().all(|(name, left_state)| {
                    right_entries.get(name).is_some_and(|right_state| {
                        path_state_hardlink_matches(left_state, right_state)
                    })
                })
        }
        (PathState::Other, PathState::Other) => true,
        _ => false,
    }
}

pub fn target_matches_source(
    source: &Path,
    target: &Path,
    current_target: &PathState,
    mode: MaterializationMode,
) -> Result<bool> {
    match mode {
        MaterializationMode::Symlink => {
            Ok(symlink_matches_expected(target, current_target, source))
        }
        MaterializationMode::Copy => {
            let source_state = snapshot_path(source)?;
            Ok(path_state_content_matches(&source_state, current_target))
        }
        MaterializationMode::Hardlink => {
            let source_state = snapshot_path(source)?;
            Ok(path_state_hardlink_matches(&source_state, current_target))
        }
    }
}

pub fn restore_from_source(
    target: &Path,
    source: &Path,
    mode: MaterializationMode,
    recorded_source_state: &PathState,
) -> Result<()> {
    let current_source_state = snapshot_path(source)?;
    if !path_state_content_matches(&current_source_state, recorded_source_state) {
        bail!(
            "refusing to undo because source {} no longer matches the recorded state",
            source.display()
        );
    }

    remove_existing_path(target)?;
    materialize_target(source, target, mode)
}

pub fn create_backup_artifact(
    store: &StateStore,
    applied_at: u64,
    index: usize,
    target: &Path,
    target_state: &PathState,
) -> Result<Option<BackupArtifact>> {
    match target_state {
        PathState::Missing => Ok(None),
        PathState::Symlink { .. } | PathState::File { .. } | PathState::Directory { .. } => {
            let backup_path = backup_path_for(store, applied_at, index, target);
            if let Some(parent) = backup_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            remove_existing_path(&backup_path)?;
            materialize_copy(target, &backup_path)?;

            Ok(Some(BackupArtifact {
                path: path_to_string(&backup_path),
                state: target_state.clone(),
            }))
        }
        PathState::Other => bail!(
            "cannot create backup artifact for unsupported content at {}",
            target.display()
        ),
    }
}

pub fn restore_from_backup(target: &Path, backup: &BackupArtifact) -> Result<()> {
    let backup_path = Path::new(&backup.path);
    let current_backup_state = snapshot_path(backup_path)?;
    if !path_state_content_matches(&current_backup_state, &backup.state) {
        bail!(
            "refusing to restore backup for {} because {} no longer matches the recorded backup state",
            target.display(),
            backup_path.display()
        );
    }

    remove_existing_path(target)?;
    materialize_copy(backup_path, target)
}

fn materialize_copy(source: &Path, target: &Path) -> Result<()> {
    materialize_tree(source, target, MaterializationMode::Copy)
}

fn materialize_hardlink(source: &Path, target: &Path) -> Result<()> {
    materialize_tree(source, target, MaterializationMode::Hardlink)
}

fn materialize_tree(source: &Path, target: &Path, mode: MaterializationMode) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("failed to inspect source {}", source.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        let raw_target = fs::read_link(source)
            .with_context(|| format!("failed to read link {}", source.display()))?;
        let resolved_target = resolved_link_target(source, &raw_target);
        let link_target = symlink_target_for(&resolved_target, target);
        return create_symlink(target, &link_target);
    }

    if file_type.is_dir() {
        fs::create_dir_all(target)
            .with_context(|| format!("failed to create {}", target.display()))?;
        apply_permissions(target, &metadata)?;
        for entry in fs::read_dir(source)
            .with_context(|| format!("failed to read directory {}", source.display()))?
        {
            let entry = entry?;
            materialize_tree(&entry.path(), &target.join(entry.file_name()), mode)?;
        }
        return Ok(());
    }

    if file_type.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        match mode {
            MaterializationMode::Copy => {
                fs::copy(source, target).with_context(|| {
                    format!(
                        "failed to copy {} to {}",
                        source.display(),
                        target.display()
                    )
                })?;
                apply_permissions(target, &metadata)?;
            }
            MaterializationMode::Hardlink => fs::hard_link(source, target).with_context(|| {
                format!(
                    "failed to hardlink {} to {}",
                    source.display(),
                    target.display()
                )
            })?,
            MaterializationMode::Symlink => unreachable!(),
        }

        return Ok(());
    }

    bail!("unsupported source type at {}", source.display())
}

fn snapshot_file(path: &Path, metadata: &fs::Metadata) -> Result<PathState> {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    let sha256 = sha256_file(path)?;
    #[cfg(unix)]
    let (device, inode) = (Some(metadata.dev()), Some(metadata.ino()));
    #[cfg(not(unix))]
    let (device, inode) = (None, None);

    Ok(PathState::File {
        size: metadata.len(),
        sha256,
        device,
        inode,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(bytes_to_hex(&hasher.finalize()))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn backup_path_for(store: &StateStore, applied_at: u64, index: usize, target: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(path_to_string(target).as_bytes());
    hasher.update(applied_at.to_le_bytes());
    hasher.update(index.to_le_bytes());
    let digest = bytes_to_hex(&hasher.finalize());

    store
        .backups_root()
        .join(applied_at.to_string())
        .join(format!("{index:04}-{digest}"))
}

fn apply_permissions(target: &Path, metadata: &fs::Metadata) -> Result<()> {
    fs::set_permissions(target, metadata.permissions())
        .with_context(|| format!("failed to set permissions on {}", target.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(path, payload).with_context(|| format!("failed to write {}", path.display()))
}
