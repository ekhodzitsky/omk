use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn compute_checksum(content: &str) -> String {
    format!("{:016x}", fnv1a_64(content.as_bytes()))
}

pub fn compute_checksum_bytes(data: &[u8]) -> String {
    format!("{:016x}", fnv1a_64(data))
}

/// Check whether `path` exists and its content is byte-identical or
/// checksum-identical to `new_content`.
pub async fn is_identical(path: &Path, new_content: &str) -> bool {
    if !path.exists() {
        return false;
    }
    match tokio::fs::read(path).await {
        Ok(existing) => {
            let new = new_content.as_bytes();
            existing == new || compute_checksum_bytes(&existing) == compute_checksum_bytes(new)
        }
        Err(_) => false,
    }
}

/// Create a backup of `path` if it exists and its content differs from `new_content`.
/// Returns the backup path on success, or None if no backup was needed or creation failed.
pub async fn maybe_backup(path: &Path, new_content: &str) -> Option<String> {
    if !path.exists() {
        return None;
    }
    if is_identical(path, new_content).await {
        return None;
    }
    let timestamp = chrono::Utc::now().timestamp();
    let backup_path = format!("{}.omk-backup-{}", path.display(), timestamp);
    match tokio::fs::copy(path, &backup_path).await {
        Ok(_) => Some(backup_path),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to create backup");
            None
        }
    }
}

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub checksum: Option<String>,
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

impl AssetManifest {
    pub fn new(project_dir: &Path) -> Self {
        Self {
            version: MANIFEST_SCHEMA_VERSION,
            created_at: chrono::Utc::now(),
            omk_version: env!("CARGO_PKG_VERSION").to_string(),
            project_dir: project_dir.to_path_buf(),
            files: Vec::new(),
            directories: Vec::new(),
        }
    }

    pub async fn add_file(&mut self, path: &Path, kind: EntryKind) {
        let abs = self.project_dir.join(path);
        let checksum = if let Ok(content) = tokio::fs::read_to_string(&abs).await {
            Some(compute_checksum(&content))
        } else {
            None
        };
        self.files.push(ManifestEntry {
            path: path.to_path_buf(),
            kind,
            checksum,
        });
    }

    pub fn add_dir(&mut self, path: &Path) {
        self.directories.push(path.to_path_buf());
    }

    pub fn manifest_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".kimi").join("omk-manifest.json")
    }

    pub async fn save(&self, project_dir: &Path) -> Result<()> {
        let path = Self::manifest_path(project_dir);
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn schema_version() -> u32 {
        MANIFEST_SCHEMA_VERSION
    }

    pub async fn load(project_dir: &Path) -> Result<Option<Self>> {
        let path = Self::manifest_path(project_dir);
        if !path.exists() {
            return Ok(None);
        }
        let json = tokio::fs::read_to_string(&path).await?;
        let manifest: Self = serde_json::from_str(&json)?;
        if manifest.version > MANIFEST_SCHEMA_VERSION {
            return Err(anyhow!(
                "Manifest version {} is newer than supported {}",
                manifest.version,
                MANIFEST_SCHEMA_VERSION
            ));
        }
        if manifest.version < MANIFEST_SCHEMA_VERSION {
            tracing::warn!(
                "Manifest version {} may be outdated; current is {}",
                manifest.version,
                MANIFEST_SCHEMA_VERSION
            );
        }
        Ok(Some(manifest))
    }

    /// Remove every file and directory recorded in this manifest.
    /// Directories are removed only if empty after file deletion.
    pub async fn rollback(&self, project_dir: &Path, dry_run: bool) -> Result<RollbackReport> {
        let mut report = RollbackReport::default();

        // Delete files
        for entry in &self.files {
            let abs = project_dir.join(&entry.path);
            if abs.exists() {
                if dry_run {
                    report.would_remove.push(entry.path.display().to_string());
                } else {
                    match tokio::fs::remove_file(&abs).await {
                        Ok(()) => report.removed_files.push(entry.path.clone()),
                        Err(e) => {
                            report
                                .errors
                                .push(format!("remove file {}: {}", abs.display(), e))
                        }
                    }
                }
            } else {
                report.already_missing.push(entry.path.clone());
            }
        }

        // Delete directories if empty, deepest first
        let mut dirs: Vec<_> = self.directories.clone();
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for dir in &dirs {
            let abs = project_dir.join(dir);
            if abs.exists() {
                if dry_run {
                    report.would_remove.push(dir.display().to_string());
                } else {
                    match tokio::fs::read_dir(&abs).await {
                        Ok(mut rd) => {
                            if rd.next_entry().await?.is_none() {
                                match tokio::fs::remove_dir(&abs).await {
                                    Ok(()) => report.removed_dirs.push(dir.clone()),
                                    Err(e) => report.errors.push(format!(
                                        "remove dir {}: {}",
                                        abs.display(),
                                        e
                                    )),
                                }
                            } else {
                                report.skipped_non_empty_dirs.push(dir.clone());
                            }
                        }
                        Err(e) => report
                            .errors
                            .push(format!("read dir {}: {}", abs.display(), e)),
                    }
                }
            }
        }

        // Remove manifest itself
        if !dry_run {
            let manifest_path = Self::manifest_path(project_dir);
            if manifest_path.exists() {
                let _ = tokio::fs::remove_file(&manifest_path).await;
            }
        }

        Ok(report)
    }

    /// Return paths in the manifest that no longer exist on disk or have checksum drift.
    pub async fn drifted_files(&self, project_dir: &Path) -> Vec<(PathBuf, Option<String>)> {
        let mut drifted = Vec::new();
        for entry in &self.files {
            let abs = project_dir.join(&entry.path);
            if !abs.exists() {
                drifted.push((entry.path.clone(), None));
            } else if let Some(ref expected) = entry.checksum {
                match tokio::fs::read_to_string(&abs).await {
                    Ok(content) => {
                        let actual = compute_checksum(&content);
                        if actual != *expected {
                            drifted.push((entry.path.clone(), Some(expected.clone())));
                        }
                    }
                    Err(_) => {
                        drifted.push((entry.path.clone(), Some(expected.clone())));
                    }
                }
            }
        }
        drifted
    }

    #[allow(dead_code)]
    pub async fn verify_checksum(
        &self,
        project_dir: &Path,
    ) -> Vec<(PathBuf, Option<String>, Option<String>)> {
        let mut drifted = Vec::new();
        for entry in &self.files {
            let abs = project_dir.join(&entry.path);
            if !abs.exists() {
                drifted.push((entry.path.clone(), entry.checksum.clone(), None));
            } else if let Some(ref expected) = entry.checksum {
                match tokio::fs::read_to_string(&abs).await {
                    Ok(content) => {
                        let actual = compute_checksum(&content);
                        if actual != *expected {
                            drifted.push((
                                entry.path.clone(),
                                Some(expected.clone()),
                                Some(actual),
                            ));
                        }
                    }
                    Err(_) => {
                        drifted.push((entry.path.clone(), Some(expected.clone()), None));
                    }
                }
            }
        }
        drifted
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compute_checksum() {
        let checksum = compute_checksum("hello");
        assert_eq!(checksum.len(), 16);
        // FNV-1a 64-bit hash of "hello"
        assert_eq!(checksum, "a430d84680aabd0b");
    }

    #[tokio::test]
    async fn test_drifted_files_missing() {
        let dir = TempDir::new().unwrap();
        let mut manifest = AssetManifest::new(dir.path());
        manifest
            .add_file(Path::new("test.txt"), EntryKind::Other)
            .await;
        let drifted = manifest.drifted_files(dir.path()).await;
        assert_eq!(drifted.len(), 1);
        assert_eq!(drifted[0].0, PathBuf::from("test.txt"));
        assert!(drifted[0].1.is_none());
    }

    #[tokio::test]
    async fn test_drifted_files_checksum_mismatch() {
        let dir = TempDir::new().unwrap();
        let mut manifest = AssetManifest::new(dir.path());
        tokio::fs::write(dir.path().join("test.txt"), "original")
            .await
            .unwrap();
        manifest
            .add_file(Path::new("test.txt"), EntryKind::Other)
            .await;

        tokio::fs::write(dir.path().join("test.txt"), "modified")
            .await
            .unwrap();
        let drifted = manifest.drifted_files(dir.path()).await;
        assert_eq!(drifted.len(), 1);
        assert_eq!(drifted[0].0, PathBuf::from("test.txt"));
        assert!(drifted[0].1.is_some());
    }

    #[tokio::test]
    async fn test_drifted_files_unchanged() {
        let dir = TempDir::new().unwrap();
        let mut manifest = AssetManifest::new(dir.path());
        tokio::fs::write(dir.path().join("test.txt"), "same")
            .await
            .unwrap();
        manifest
            .add_file(Path::new("test.txt"), EntryKind::Other)
            .await;
        let drifted = manifest.drifted_files(dir.path()).await;
        assert!(drifted.is_empty());
    }

    #[tokio::test]
    async fn test_verify_checksum_missing() {
        let dir = TempDir::new().unwrap();
        let mut manifest = AssetManifest::new(dir.path());
        tokio::fs::write(dir.path().join("test.txt"), "content")
            .await
            .unwrap();
        manifest
            .add_file(Path::new("test.txt"), EntryKind::Other)
            .await;
        tokio::fs::remove_file(dir.path().join("test.txt"))
            .await
            .unwrap();

        let verified = manifest.verify_checksum(dir.path()).await;
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].0, PathBuf::from("test.txt"));
        assert!(verified[0].1.is_some()); // expected
        assert!(verified[0].2.is_none()); // actual
    }

    #[tokio::test]
    async fn test_verify_checksum_mismatch() {
        let dir = TempDir::new().unwrap();
        let mut manifest = AssetManifest::new(dir.path());
        tokio::fs::write(dir.path().join("test.txt"), "original")
            .await
            .unwrap();
        manifest
            .add_file(Path::new("test.txt"), EntryKind::Other)
            .await;

        tokio::fs::write(dir.path().join("test.txt"), "modified")
            .await
            .unwrap();
        let verified = manifest.verify_checksum(dir.path()).await;
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].0, PathBuf::from("test.txt"));
        assert!(verified[0].1.is_some()); // expected
        assert!(verified[0].2.is_some()); // actual
        assert_ne!(verified[0].1, verified[0].2);
    }

    #[tokio::test]
    async fn test_manifest_schema_version_too_new() {
        let dir = TempDir::new().unwrap();
        tokio::fs::create_dir_all(dir.path().join(".kimi"))
            .await
            .unwrap();
        let manifest = AssetManifest::new(dir.path());
        manifest.save(dir.path()).await.unwrap();

        // Modify the manifest on disk to have a future version
        let path = AssetManifest::manifest_path(dir.path());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let content = content.replace("\"version\": 1", "\"version\": 999");
        tokio::fs::write(&path, content).await.unwrap();

        let result = AssetManifest::load(dir.path()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("newer than supported"));
    }

    #[tokio::test]
    async fn test_manifest_schema_version_outdated() {
        let dir = TempDir::new().unwrap();
        tokio::fs::create_dir_all(dir.path().join(".kimi"))
            .await
            .unwrap();
        let manifest = AssetManifest::new(dir.path());
        manifest.save(dir.path()).await.unwrap();

        let path = AssetManifest::manifest_path(dir.path());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let content = content.replace("\"version\": 1", "\"version\": 0");
        tokio::fs::write(&path, content).await.unwrap();

        let result = AssetManifest::load(dir.path()).await;
        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert!(loaded.is_some());
    }
}
