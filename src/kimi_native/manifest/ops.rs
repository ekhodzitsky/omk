use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use super::checksum::compute_checksum;
use super::path::{to_project_relative, validate_manifest_paths};
use super::types::{
    AssetManifest, BackupEntry, EntryKind, ManifestEntry, RollbackReport, MANIFEST_SCHEMA_VERSION,
};

impl AssetManifest {
    pub fn new(project_dir: &Path) -> Self {
        Self {
            version: MANIFEST_SCHEMA_VERSION,
            created_at: chrono::Utc::now(),
            omk_version: env!("CARGO_PKG_VERSION").to_string(),
            project_dir: project_dir.to_path_buf(),
            files: Vec::new(),
            directories: Vec::new(),
            backups: Vec::new(),
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

    /// Record backup metadata for a managed file.
    /// Stores project-relative paths so index remains portable.
    pub fn add_backup(&mut self, managed_path: &Path, backup_path: &Path) {
        let managed_rel = to_project_relative(managed_path, &self.project_dir);
        let backup_rel = to_project_relative(backup_path, &self.project_dir);
        if let (Some(managed_path), Some(backup_path)) = (managed_rel, backup_rel) {
            self.backups.push(BackupEntry {
                managed_path,
                backup_path,
                created_at: chrono::Utc::now(),
            });
        } else {
            tracing::warn!(
                managed = %managed_path.display(),
                backup = %backup_path.display(),
                "Skipping backup index entry outside project root"
            );
        }
    }

    pub fn latest_backup_for(&self, managed_path: &Path) -> Option<PathBuf> {
        self.backups
            .iter()
            .filter(|entry| entry.managed_path == managed_path)
            .max_by_key(|entry| entry.created_at)
            .map(|entry| entry.backup_path.clone())
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
        validate_manifest_paths(&manifest, project_dir)?;
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
