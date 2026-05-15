use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Records every file and directory that OMK owns under `.kimi/` so that
/// `doctor`, `sync`, and `rollback` can reason precisely about what was installed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetManifest {
    pub version: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub omk_version: String,
    pub project_dir: PathBuf,
    /// Files relative to project_dir that OMK created or manages.
    pub files: Vec<ManifestEntry>,
    /// Directories that OMK created (for cleanup only when empty).
    pub directories: Vec<PathBuf>,
    /// Backup artifacts mapped to their managed file paths.
    #[serde(default)]
    pub backups: Vec<BackupEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub managed_path: PathBuf,
    pub backup_path: PathBuf,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    AgentSpec,
    AgentPrompt,
    HookScript,
    HookConfig,
    Skill,
    Config,
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct RollbackReport {
    pub removed_files: Vec<PathBuf>,
    pub removed_dirs: Vec<PathBuf>,
    pub already_missing: Vec<PathBuf>,
    pub skipped_non_empty_dirs: Vec<PathBuf>,
    pub errors: Vec<String>,
    pub would_remove: Vec<String>,
}
