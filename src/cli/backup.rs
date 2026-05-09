use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: BackupCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum BackupCommands {
    /// Create a backup of all state
    Create {
        /// Optional backup name (defaults to timestamp)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// List available backups
    List,
    /// Restore from a backup
    Restore {
        /// Backup name or path
        name: String,
    },
    /// Remove old backups, keeping only the N most recent
    Prune {
        /// Number of backups to keep
        #[arg(short, long, default_value = "5")]
        keep: usize,
    },
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        BackupCommands::Create { name } => create_backup(name).await,
        BackupCommands::List => list_backups().await,
        BackupCommands::Restore { name } => restore_backup(&name).await,
        BackupCommands::Prune { keep } => prune_backups(keep).await,
    }
}

async fn create_backup(name: Option<String>) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir();
    let backup_dir = crate::runtime::config::data_dir().join("backups");
    tokio::fs::create_dir_all(&backup_dir).await?;

    let backup_name =
        name.unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string());
    let backup_path = backup_dir.join(format!("omk-backup-{}.tar.gz", backup_name));

    info!(source = %state_dir.display(), target = %backup_path.display(), "Creating backup");
    println!("Creating backup: {}", backup_path.display());

    let status = Command::new("tar")
        .args(["-czf"])
        .arg(&backup_path)
        .arg("-C")
        .arg(state_dir.parent().unwrap_or(std::path::Path::new(".")))
        .arg(state_dir.file_name().unwrap_or_default())
        .status()
        .await
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar command failed");
    }

    let metadata = tokio::fs::metadata(&backup_path).await?;
    println!(
        "✓ Backup created: {} ({:.1} MB)",
        backup_path.display(),
        metadata.len() as f64 / 1_048_576.0
    );

    Ok(())
}

async fn list_backups() -> Result<()> {
    let backup_dir = crate::runtime::config::data_dir().join("backups");

    if !backup_dir.exists() {
        println!("No backups found.");
        return Ok(());
    }

    println!("Available backups:");
    println!();

    let mut entries = tokio::fs::read_dir(&backup_dir).await?;
    let mut found = false;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("gz") {
            found = true;
            let metadata = entry.metadata().await?;
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let modified = metadata.modified()?;
            let age = modified.elapsed().unwrap_or_default();
            let size = metadata.len() as f64 / 1_048_576.0;

            println!(
                "  {:40} {:6.1} MB  {} ago",
                name,
                size,
                humantime::format_duration(age)
            );
        }
    }

    if !found {
        println!("  No backups found.");
    }

    Ok(())
}

async fn restore_backup(name: &str) -> Result<()> {
    let backup_dir = crate::runtime::config::data_dir().join("backups");

    let backup_path = if name.contains('/') || name.ends_with(".tar.gz") {
        PathBuf::from(name)
    } else {
        backup_dir.join(format!("omk-backup-{}.tar.gz", name))
    };

    if !backup_path.exists() {
        anyhow::bail!("Backup not found: {}", backup_path.display());
    }

    println!("Restoring from: {}", backup_path.display());
    println!("⚠️  This will overwrite current state.");
    println!("Type 'yes' to confirm: ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim() != "yes" {
        println!("Aborted.");
        return Ok(());
    }

    let state_dir = crate::runtime::config::state_dir();

    // Remove current state
    if state_dir.exists() {
        tokio::fs::remove_dir_all(&state_dir).await?;
    }

    // Extract backup
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&backup_path)
        .arg("-C")
        .arg(state_dir.parent().unwrap_or(std::path::Path::new(".")))
        .status()
        .await
        .context("Failed to run tar command")?;

    if !status.success() {
        anyhow::bail!("tar extraction failed");
    }

    println!("✓ State restored from {}", backup_path.display());
    Ok(())
}

async fn prune_backups(keep: usize) -> Result<()> {
    let backup_dir = crate::runtime::config::data_dir().join("backups");

    if !backup_dir.exists() {
        println!("No backups found.");
        return Ok(());
    }

    let mut backups: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    let mut entries = tokio::fs::read_dir(&backup_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("gz") {
            let metadata = entry.metadata().await?;
            if let Ok(modified) = metadata.modified() {
                backups.push((path, modified));
            }
        }
    }

    if backups.len() <= keep {
        println!(
            "Found {} backup(s), keeping all (limit: {})",
            backups.len(),
            keep
        );
        return Ok(());
    }

    // Sort by modification time, newest first
    backups.sort_by_key(|b| std::cmp::Reverse(b.1));

    let to_remove = &backups[keep..];
    let mut removed = 0;
    let mut freed: u64 = 0;

    for (path, _) in to_remove {
        let metadata = tokio::fs::metadata(path).await?;
        let size = metadata.len();
        tokio::fs::remove_file(path).await?;
        info!(path = %path.display(), "Removed old backup");
        removed += 1;
        freed += size;
    }

    println!(
        "✓ Pruned {} old backup(s) ({:.1} MB freed), keeping {} most recent",
        removed,
        freed as f64 / 1_048_576.0,
        keep
    );

    Ok(())
}
