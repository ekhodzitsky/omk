use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: SkillCommands,
}

#[derive(Subcommand, Debug)]
pub enum SkillCommands {
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
    /// Remove an installed skill
    Remove {
        /// Skill name
        name: String,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        SkillCommands::Install { url, name } => install_skill(&url, name).await,
        SkillCommands::List => list_skills().await,
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

    let target_dir = skills_dir.join(&skill_name);

    if target_dir.exists() {
        anyhow::bail!("Skill '{}' already exists. Remove it first.", skill_name);
    }

    if url.starts_with("http") || url.starts_with("git@") {
        info!(url = %url, name = %skill_name, "Cloning skill from git");
        println!("Installing skill '{}' from {}...", skill_name, url);

        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", url, target_dir.to_str().unwrap()])
            .output()
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
        println!("Installing skill '{}' from {}...", skill_name, source.display());

        copy_dir(&source, &target_dir).await?;
    }

    println!("✓ Installed skill '{}' to {}", skill_name, target_dir.display());
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
            let name = path.file_name().unwrap().to_string_lossy();
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

async fn remove_skill(name: &str) -> Result<()> {
    let skills_dir = crate::runtime::config::data_dir().join("skills");
    let target = skills_dir.join(name);

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
