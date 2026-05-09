use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::warn;

use super::manifest::{compute_checksum, AssetManifest};

/// Report produced by a rollback operation.
#[derive(Debug, Clone, Default)]
pub struct RollbackReport {
    /// Rollback was requested but no manifest was found (clean no-op).
    pub manifest_missing: bool,
    /// Files restored from a backup.
    pub restored: Vec<PathBuf>,
    /// Files or directories removed (OMK-created, no backup).
    pub removed: Vec<PathBuf>,
    /// Files or directories skipped (missing, user-modified, or non-empty dir).
    pub skipped: Vec<PathBuf>,
    /// Errors encountered during rollback.
    pub errors: Vec<String>,
}

/// Rollback OMK-managed assets.
/// If no OMK manifest exists, returns a clean no-op report with `manifest_missing=true`.
///
/// For each managed file:
/// - If a backup exists: restore from backup, delete backup.
/// - If no backup and file matches manifest checksum (OMK-created): delete file.
/// - If no backup and file doesn't match manifest checksum (user-modified): skip with warning.
/// - If file is missing: skip.
pub async fn rollback(project_dir: &Path, dry_run: bool) -> Result<RollbackReport> {
    let manifest = match AssetManifest::load(project_dir).await? {
        Some(m) => m,
        None => {
            return Ok(RollbackReport {
                manifest_missing: true,
                ..RollbackReport::default()
            });
        }
    };

    let mut report = RollbackReport::default();

    // Process files
    for entry in &manifest.files {
        let abs = project_dir.join(&entry.path);
        let backup = find_backup_for(&abs).await;

        if let Some(ref backup_path) = backup {
            if dry_run {
                report.restored.push(entry.path.clone());
            } else {
                match tokio::fs::copy(backup_path, &abs).await {
                    Ok(_) => {
                        if let Err(e) = tokio::fs::remove_file(backup_path).await {
                            warn!(
                                path = %backup_path.display(),
                                error = %e,
                                "Failed to remove backup file"
                            );
                        }
                        report.restored.push(entry.path.clone());
                    }
                    Err(e) => {
                        report.errors.push(format!(
                            "restore {} from backup {}: {}",
                            abs.display(),
                            backup_path.display(),
                            e
                        ));
                    }
                }
            }
        } else if abs.exists() {
            let should_remove = match entry.checksum {
                Some(ref expected) => match tokio::fs::read_to_string(&abs).await {
                    Ok(content) => {
                        let actual = compute_checksum(&content);
                        actual == *expected
                    }
                    Err(_) => false,
                },
                None => false,
            };

            if should_remove {
                if dry_run {
                    report.removed.push(entry.path.clone());
                } else {
                    match tokio::fs::remove_file(&abs).await {
                        Ok(()) => report.removed.push(entry.path.clone()),
                        Err(e) => {
                            report
                                .errors
                                .push(format!("remove {}: {}", abs.display(), e));
                        }
                    }
                }
            } else {
                report.skipped.push(entry.path.clone());
            }
        } else {
            report.skipped.push(entry.path.clone());
        }
    }

    // Process directories — remove only if empty, deepest first
    let mut dirs: Vec<_> = manifest.directories.clone();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));

    for dir in &dirs {
        let abs = project_dir.join(dir);
        if abs.exists() {
            if dry_run {
                // In dry-run we can't know for sure if the dir will end up empty,
                // so we only report it if it currently looks empty.
                match tokio::fs::read_dir(&abs).await {
                    Ok(mut rd) => match rd.next_entry().await {
                        Ok(None) => report.removed.push(dir.clone()),
                        Ok(Some(_)) => report.skipped.push(dir.clone()),
                        Err(e) => report
                            .errors
                            .push(format!("read dir {}: {}", abs.display(), e)),
                    },
                    Err(e) => {
                        report
                            .errors
                            .push(format!("read dir {}: {}", abs.display(), e));
                    }
                }
            } else {
                match tokio::fs::read_dir(&abs).await {
                    Ok(mut rd) => {
                        if rd.next_entry().await?.is_none() {
                            match tokio::fs::remove_dir(&abs).await {
                                Ok(()) => report.removed.push(dir.clone()),
                                Err(e) => {
                                    report.errors.push(format!(
                                        "remove dir {}: {}",
                                        abs.display(),
                                        e
                                    ));
                                }
                            }
                        } else {
                            report.skipped.push(dir.clone());
                        }
                    }
                    Err(e) => {
                        report
                            .errors
                            .push(format!("read dir {}: {}", abs.display(), e));
                    }
                }
            }
        } else {
            report.skipped.push(dir.clone());
        }
    }

    // Remove manifest itself
    if !dry_run {
        let manifest_path = AssetManifest::manifest_path(project_dir);
        if manifest_path.exists() {
            let _ = tokio::fs::remove_file(&manifest_path).await;
        }
    }

    Ok(report)
}

/// Find the most recent backup file for a given path.
/// Backups follow the pattern `{path}.omk-backup-{timestamp}`.
async fn find_backup_for(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let file_name = path.file_name()?.to_string_lossy();
    let prefix = format!("{}.omk-backup-", file_name);

    let mut entries = tokio::fs::read_dir(parent).await.ok()?;
    let mut backups = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            backups.push(entry.path());
        }
    }

    // Sort lexicographically; the timestamp suffix makes the last entry the most recent.
    backups.sort();
    backups.into_iter().next_back()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_rollback_no_manifest() {
        let dir = TempDir::new().unwrap();
        let report = rollback(dir.path(), false).await.unwrap();
        assert!(report.manifest_missing);
        assert!(report.restored.is_empty());
        assert!(report.removed.is_empty());
        assert!(report.skipped.is_empty());
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_rollback_removes_omk_created_file() {
        let dir = TempDir::new().unwrap();
        tokio::fs::create_dir_all(dir.path().join(".kimi"))
            .await
            .unwrap();
        let mut manifest = AssetManifest::new(dir.path());

        // Create a file that OMK "owns" (checksum matches manifest)
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "omk-content").await.unwrap();
        manifest
            .add_file(
                std::path::Path::new("test.txt"),
                super::super::manifest::EntryKind::Other,
            )
            .await;
        manifest.save(dir.path()).await.unwrap();

        let report = rollback(dir.path(), false).await.unwrap();
        assert!(!file_path.exists());
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.removed[0], PathBuf::from("test.txt"));
    }

    #[tokio::test]
    async fn test_rollback_skips_user_modified_file() {
        let dir = TempDir::new().unwrap();
        tokio::fs::create_dir_all(dir.path().join(".kimi"))
            .await
            .unwrap();
        let mut manifest = AssetManifest::new(dir.path());

        // Create a file and record its checksum in the manifest
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();
        manifest
            .add_file(
                std::path::Path::new("test.txt"),
                super::super::manifest::EntryKind::Other,
            )
            .await;
        manifest.save(dir.path()).await.unwrap();

        // User modifies the file
        tokio::fs::write(&file_path, "modified").await.unwrap();

        let report = rollback(dir.path(), false).await.unwrap();
        assert!(file_path.exists());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0], PathBuf::from("test.txt"));
    }

    #[tokio::test]
    async fn test_rollback_restores_from_backup() {
        let dir = TempDir::new().unwrap();
        tokio::fs::create_dir_all(dir.path().join(".kimi"))
            .await
            .unwrap();
        let mut manifest = AssetManifest::new(dir.path());

        // Simulate: file existed, OMK backed it up, then overwrote it
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "user-content").await.unwrap();
        let backup_path = format!("{}.omk-backup-1234567890", file_path.display());
        tokio::fs::write(&backup_path, "user-content")
            .await
            .unwrap();

        // Now file has OMK content
        tokio::fs::write(&file_path, "omk-content").await.unwrap();
        manifest
            .add_file(
                std::path::Path::new("test.txt"),
                super::super::manifest::EntryKind::Other,
            )
            .await;
        manifest.save(dir.path()).await.unwrap();

        let report = rollback(dir.path(), false).await.unwrap();
        assert_eq!(report.restored.len(), 1);
        assert_eq!(report.restored[0], PathBuf::from("test.txt"));

        // File should have been restored from backup
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "user-content");

        // Backup should be deleted
        assert!(!std::path::Path::new(&backup_path).exists());
    }

    #[tokio::test]
    async fn test_rollback_dry_run_no_changes() {
        let dir = TempDir::new().unwrap();
        tokio::fs::create_dir_all(dir.path().join(".kimi"))
            .await
            .unwrap();
        let mut manifest = AssetManifest::new(dir.path());

        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "omk-content").await.unwrap();
        manifest
            .add_file(
                std::path::Path::new("test.txt"),
                super::super::manifest::EntryKind::Other,
            )
            .await;
        manifest.save(dir.path()).await.unwrap();

        let report = rollback(dir.path(), true).await.unwrap();
        assert_eq!(report.removed.len(), 1);
        assert!(file_path.exists()); // file still there
    }
}
