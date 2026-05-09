use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use crate::runtime::sanitize::sanitize_name;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub command: SkillCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SkillCommands {
    /// Install a skill from a git repository
    Install {
        /// Git URL or local path
        url: String,
        /// Optional name override
        #[arg(short, long)]
        name: Option<String>,
    },
    /// List installed skills
    List,
    /// Show a skill's contents
    Show {
        /// Skill name
        name: String,
    },
    /// Search installed skills
    Search {
        /// Search query
        query: String,
    },
    /// Remove an installed skill
    Remove {
        /// Skill name
        name: String,
    },
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        SkillCommands::Install { url, name } => install_skill(&url, name).await,
        SkillCommands::List => list_skills().await,
        SkillCommands::Show { name } => show_skill(&name).await,
        SkillCommands::Search { query } => search_skills(&query).await,
        SkillCommands::Remove { name } => remove_skill(&name).await,
    }
}

async fn install_skill(url: &str, name_override: Option<String>) -> Result<()> {
    let skills_dir = crate::runtime::config::data_dir().join("skills");
    tokio::fs::create_dir_all(&skills_dir).await?;

    let skill_name = name_override.unwrap_or_else(|| {
        url.trim_end_matches('/')
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .to_string()
    });
    let skill_name = sanitize_name(&skill_name)?;

    let target_dir = skills_dir.join(&skill_name);

    if target_dir.exists() {
        anyhow::bail!("Skill '{}' already exists. Remove it first.", skill_name);
    }

    if url.starts_with("http") || url.starts_with("git@") {
        info!(url = %url, name = %skill_name, "Cloning skill from git");
        println!("Installing skill '{}' from {}...", skill_name, url);

        let output = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", url])
            .arg(&target_dir)
            .output()
            .await
            .context("git is required to install skills from URLs")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git clone failed: {}", stderr);
        }
    } else {
        let source = std::path::PathBuf::from(url);
        if !source.exists() {
            anyhow::bail!("Source path does not exist: {}", source.display());
        }

        info!(source = %source.display(), target = %target_dir.display(), "Copying skill");
        println!(
            "Installing skill '{}' from {}...",
            skill_name,
            source.display()
        );

        copy_dir(&source, &target_dir).await?;
    }

    println!(
        "✓ Installed skill '{}' to {}",
        skill_name,
        target_dir.display()
    );
    Ok(())
}

async fn list_skills() -> Result<()> {
    let skills_dir = crate::runtime::config::data_dir().join("skills");

    if !skills_dir.exists() {
        println!("No skills installed.");
        return Ok(());
    }

    println!("Installed skills:");
    println!();

    let mut entries = tokio::fs::read_dir(&skills_dir).await?;
    let mut found = false;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            found = true;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_else(|| std::borrow::Cow::from("unknown"));
            let has_skill_md = path.join("SKILL.md").exists();
            let indicator = if has_skill_md { "✓" } else { "⚠" };
            println!("  {} {} ({})", indicator, name, path.display());
        }
    }

    if !found {
        println!("  No skills installed.");
    }

    Ok(())
}

async fn show_skill(name: &str) -> Result<()> {
    let skills_dir = crate::runtime::config::data_dir().join("skills");
    let name = sanitize_name(name)?;
    let target = skills_dir.join(&name);

    if !target.exists() {
        anyhow::bail!("Skill '{}' not found.", name);
    }

    let skill_md = target.join("SKILL.md");
    if skill_md.exists() {
        let content = tokio::fs::read_to_string(&skill_md).await?;
        println!("=== {} ===\n", skill_md.display());
        println!("{}", content);
    } else {
        println!("Skill '{}' (no SKILL.md found)", name);
        println!("Path: {}", target.display());
    }

    Ok(())
}

async fn search_skills(query: &str) -> Result<()> {
    let skills_dir = crate::runtime::config::data_dir().join("skills");

    if !skills_dir.exists() {
        println!("No skills installed.");
        return Ok(());
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();
    let mut entries = tokio::fs::read_dir(&skills_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let skill_md = path.join("SKILL.md");
            let mut content = String::new();
            if skill_md.exists() {
                content = tokio::fs::read_to_string(&skill_md)
                    .await
                    .unwrap_or_default();
            }
            if name.to_lowercase().contains(&query_lower)
                || content.to_lowercase().contains(&query_lower)
            {
                matches.push((name, path, !content.is_empty()));
            }
        }
    }

    if matches.is_empty() {
        println!("No skills found for '{}'", query);
        return Ok(());
    }

    println!("Found {} skill(s) for '{}':\n", matches.len(), query);
    for (name, path, has_md) in matches {
        let indicator = if has_md { "✓" } else { "⚠" };
        println!("  {} {} ({})", indicator, name, path.display());
    }

    Ok(())
}

async fn remove_skill(name: &str) -> Result<()> {
    let skills_dir = crate::runtime::config::data_dir().join("skills");
    let name = sanitize_name(name)?;
    let target = skills_dir.join(&name);

    if !target.exists() {
        anyhow::bail!("Skill '{}' not found.", name);
    }

    tokio::fs::remove_dir_all(&target).await?;
    println!("✓ Removed skill '{}'", name);
    Ok(())
}

async fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut stack: Vec<(std::path::PathBuf, std::path::PathBuf)> =
        vec![(src.to_path_buf(), dst.to_path_buf())];

    while let Some((current_src, current_dst)) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current_src).await?;

        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = current_dst.join(entry.file_name());

            if src_path.is_dir() {
                tokio::fs::create_dir_all(&dst_path).await?;
                stack.push((src_path, dst_path));
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
    }

    Ok(())
}
