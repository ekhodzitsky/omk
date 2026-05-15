use std::path::Path;

use anyhow::Result;

use crate::kimi_native::diagnostics::{DiagResult, Severity};

pub(super) async fn check_skills(kimi_dir: &Path, results: &mut Vec<DiagResult>) -> Result<()> {
    // Check skills paths (L1-034)
    let skills_dir = kimi_dir.join("skills");
    if skills_dir.exists() {
        if skills_dir.is_dir() {
            let mut invalid = vec![];
            let mut validated = 0usize;
            let mut entries = tokio::fs::read_dir(&skills_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let entry_name = entry.file_name().to_string_lossy().to_string();
                let file_type = entry.file_type().await?;

                if !file_type.is_dir() {
                    invalid.push(format!("{} is not a directory", entry_path.display()));
                    continue;
                }

                let skill_md = entry_path.join("SKILL.md");
                if !skill_md.exists() {
                    invalid.push(format!("{}/SKILL.md is missing", entry_name));
                    continue;
                }

                validated += 1;
            }

            if invalid.is_empty() {
                results.push(DiagResult {
                    severity: Severity::Ok,
                    message: format!("Skills paths valid ({} skill(s))", validated),
                    fix_hint: None,
                });
            } else {
                results.push(DiagResult {
                    severity: Severity::Warning,
                    message: format!("Invalid skills paths: {}", invalid.join(", ")),
                    fix_hint: Some(
                        "Ensure each .kimi/skills/<name>/SKILL.md exists, or run `omk kimi sync`"
                            .to_string(),
                    ),
                });
            }
        } else {
            results.push(DiagResult {
                severity: Severity::Warning,
                message: format!(
                    "Invalid skills path: {} is not a directory",
                    skills_dir.display()
                ),
                fix_hint: Some("Remove it and run `omk kimi sync`".to_string()),
            });
        }
    }

    Ok(())
}
