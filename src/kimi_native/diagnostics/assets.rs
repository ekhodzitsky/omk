use std::path::Path;

use crate::kimi_native::diagnostics::{DiagResult, Severity};

pub(super) fn check_agents_md(dir: &Path, results: &mut Vec<DiagResult>) {
    // Check for AGENTS.md
    let agents_md = dir.join("AGENTS.md");
    if agents_md.exists() {
        results.push(DiagResult {
            severity: Severity::Ok,
            message: "AGENTS.md found".to_string(),
            fix_hint: None,
        });
    } else {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: "AGENTS.md not found".to_string(),
            fix_hint: Some("Create AGENTS.md with project conventions".to_string()),
        });
    }
}

pub(super) async fn check_manifest(dir: &Path, results: &mut Vec<DiagResult>) {
    // Check for asset manifest
    let manifest_path = dir.join(".kimi").join("omk-manifest.json");
    if manifest_path.exists() {
        match crate::kimi_native::manifest::AssetManifest::load(dir).await {
            Ok(Some(manifest)) => {
                results.push(DiagResult {
                    severity: Severity::Ok,
                    message: format!(
                        "Asset manifest found (OMK v{}, {} files)",
                        manifest.omk_version,
                        manifest.files.len()
                    ),
                    fix_hint: None,
                });
                let drifted = manifest.drifted_files(dir).await;
                for (path, expected) in drifted {
                    match expected {
                        None => {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!("Missing manifest file: {}", path.display()),
                                fix_hint: Some(
                                    "Run `omk kimi sync` to restore missing files".to_string(),
                                ),
                            });
                        }
                        Some(checksum) => {
                            results.push(DiagResult {
                                severity: Severity::Warning,
                                message: format!(
                                    "File content drift detected: {} (expected: {})",
                                    path.display(),
                                    checksum
                                ),
                                fix_hint: Some(
                                    "Run `omk kimi sync` to restore drifted files".to_string(),
                                ),
                            });
                        }
                    }
                }

                if !manifest.backups.is_empty() {
                    results.push(DiagResult {
                        severity: Severity::Ok,
                        message: format!("Backup index entries: {}", manifest.backups.len()),
                        fix_hint: None,
                    });
                }

                for backup in &manifest.backups {
                    let backup_abs = dir.join(&backup.backup_path);
                    if !backup_abs.exists() {
                        results.push(DiagResult {
                            severity: Severity::Warning,
                            message: format!(
                                "Backup index drift: managed '{}' points to missing backup '{}'",
                                backup.managed_path.display(),
                                backup.backup_path.display()
                            ),
                            fix_hint: Some(
                                "Run `omk kimi sync --force` to refresh managed assets and backup index"
                                    .to_string(),
                            ),
                        });
                    }
                }
            }
            Ok(None) => {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: "Asset manifest not found".to_string(),
                    fix_hint: Some("Run `omk kimi sync` to generate a manifest".to_string()),
                });
            }
            Err(e) => {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: format!("Cannot read asset manifest: {}", e),
                    fix_hint: None,
                });
            }
        }
    } else {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: "Asset manifest not found".to_string(),
            fix_hint: Some("Run `omk kimi sync` to generate a manifest".to_string()),
        });
    }
}
