#![allow(dead_code)]

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use super::parser::{parse_skill, Skill};

/// Load a bundled skill by name from the built-in skills directory.
/// Falls back to CARGO_MANIFEST_DIR/skills/<name>/SKILL.md
pub async fn load_bundled_skill(name: &str) -> Result<String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::current_exe().map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
        })
        .context("Cannot determine omk install directory")?;

    let skill_path = manifest_dir.join("skills").join(name).join("SKILL.md");
    if skill_path.exists() {
        tokio::fs::read_to_string(&skill_path)
            .await
            .with_context(|| format!("Failed to read skill: {}", skill_path.display()))
    } else {
        anyhow::bail!(
            "Bundled skill '{}' not found at: {}",
            name,
            skill_path.display()
        )
    }
}

/// Discover skills from multiple directories in priority order:
/// 1. Project scope: .omk/skills/
/// 2. User scope: ~/.omk/skills/
/// 3. System/bundled: <omk binary dir>/skills/
pub async fn discover_skills(project_root: Option<&Path>) -> Result<Vec<Skill>> {
    let mut skills: Vec<Skill> = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    let mut dirs: Vec<PathBuf> = vec![];

    // Project scope
    if let Some(root) = project_root {
        dirs.push(root.join(".omk").join("skills"));
    }

    // User scope (XDG or legacy)
    dirs.push(crate::runtime::config::data_dir().join("skills"));

    // Bundled skills (relative to binary — for dev, use CARGO_MANIFEST_DIR)
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        dirs.push(PathBuf::from(manifest).join("skills"));
    }

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        debug!(dir = %dir.display(), "Scanning skills directory");

        match scan_skill_dir(&dir).await {
            Ok(found) => {
                for skill in found {
                    if seen_names.insert(skill.name.clone()) {
                        skills.push(skill);
                    } else {
                        debug!(name = %skill.name, "Skipping duplicate skill");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "Failed to scan skills directory");
            }
        }
    }

    info!(count = skills.len(), "Discovered skills");
    Ok(skills)
}

async fn scan_skill_dir(dir: &Path) -> Result<Vec<Skill>> {
    let mut skills = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                match parse_skill(&skill_md).await {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to parse skill")
                    }
                }
            }
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            // Flat skill file
            match parse_skill(&path).await {
                Ok(skill) => skills.push(skill),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to parse skill")
                }
            }
        }
    }

    Ok(skills)
}

/// Find a skill by name or alias
pub fn find_skill<'a>(skills: &'a [Skill], name: &str) -> Option<&'a Skill> {
    let name_lower = name.to_lowercase();
    skills.iter().find(|s| {
        s.name.to_lowercase() == name_lower
            || s.aliases.iter().any(|a| a.to_lowercase() == name_lower)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_bundled_team_skill() {
        // This test only works when CARGO_MANIFEST_DIR is set (cargo test)
        let result = load_bundled_skill("team").await;
        assert!(
            result.is_ok(),
            "Failed to load bundled team skill: {:?}",
            result
        );
        let content = result.unwrap();
        assert!(
            content.contains("Team Mode"),
            "Skill content missing expected header"
        );
    }
}
