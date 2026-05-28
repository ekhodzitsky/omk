use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::process::Command;
use tracing::info;

#[derive(Parser, Debug)]
pub struct Args {
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

    let status = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new("tar")
            .args(["-czf"])
            .arg(&backup_path)
            .arg("-C")
            .arg(state_dir.parent().unwrap_or(std::path::Path::new(".")))
            .arg(state_dir.file_name().unwrap_or_default())
            .status(),
    )
    .await
    .context("tar command timed out")?
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

    let sanitized = crate::runtime::sanitize::sanitize_name(name)?;
    let backup_path = backup_dir.join(format!("omk-backup-{}.tar.gz", sanitized));

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
    let state_parent = state_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("state_dir has no parent directory"))?
        .to_path_buf();
    let state_name = state_dir
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("state_dir has no file name component"))?
        .to_os_string();

    // Extract into an isolated temp sibling first so a corrupt or hostile
    // tarball cannot wipe live state before extraction completes successfully.
    // Suffix the path with the process id so two concurrent `omk restore`
    // invocations in the same wall-clock second cannot collide and stomp on
    // each other's in-flight sandbox.
    let stamp = format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        std::process::id()
    );
    let tmp_dir = state_parent.join(format!(".omk-restore-{}", stamp));
    if tmp_dir.exists() {
        tokio::fs::remove_dir_all(&tmp_dir).await?;
    }
    tokio::fs::create_dir_all(&tmp_dir).await?;

    // --no-same-owner: do not preserve archive uid/gid (avoids surprises when
    // restoring under a different user).
    // -C tmp_dir: extract into the sandbox directory. Both GNU tar and BSD tar
    // strip leading `/` and reject `..` in member paths by default, so the
    // sandbox additionally bounds any pathological archive.
    let extract_status = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new("tar")
            .arg("--no-same-owner")
            .arg("-xzf")
            .arg(&backup_path)
            .arg("-C")
            .arg(&tmp_dir)
            .status(),
    )
    .await
    .context("tar command timed out")?
    .context("Failed to run tar command")?;

    if !extract_status.success() {
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        anyhow::bail!("tar extraction failed; live state untouched");
    }

    let extracted = tmp_dir.join(&state_name);
    if !extracted.exists() {
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        anyhow::bail!(
            "Backup did not contain expected '{}' directory at the top level; live state untouched",
            state_name.to_string_lossy()
        );
    }

    // Stash live state aside so the swap is reversible on failure.
    let stash = state_parent.join(format!(".omk-prev-{}", stamp));
    let live_existed = state_dir.exists();
    if live_existed {
        tokio::fs::rename(&state_dir, &stash).await?;
    }

    if let Err(e) = tokio::fs::rename(&extracted, &state_dir).await {
        // Roll back so the user is not left without state.
        if live_existed && stash.exists() {
            let _ = tokio::fs::rename(&stash, &state_dir).await;
        }
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
        return Err(e).context("Failed to install restored state");
    }

    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    if stash.exists() {
        let _ = tokio::fs::remove_dir_all(&stash).await;
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
