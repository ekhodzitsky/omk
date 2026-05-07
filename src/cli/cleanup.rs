use anyhow::Result;
use clap::Parser;
use std::path::Path;
use tracing::info;

#[derive(Parser, Debug)]
pub struct Args {
    /// Remove all state (teams, autopilot, ralph)
    #[arg(long)]
    all: bool,

    /// Remove state older than N days
    #[arg(long, value_name = "DAYS")]
    older_than: Option<u64>,

    /// Dry run: show what would be removed
    #[arg(long)]
    dry_run: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir();

    if args.all {
        println!("This will remove ALL omk state.");
        if !args.dry_run {
            println!("Are you sure? Type 'yes' to confirm: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim() != "yes" {
                println!("Aborted.");
                return Ok(());
            }
            tokio::fs::remove_dir_all(&state_dir).await?;
            tokio::fs::create_dir_all(&state_dir).await?;
            println!("✓ All state removed");
            return Ok(());
        } else {
            println!("Would remove: {}", state_dir.display());
            return Ok(());
        }
    }

    let mut removed = 0;
    let mut freed: u64 = 0;

    // Scan team states
    let teams_dir = state_dir.join("team");
    if teams_dir.exists() {
        let mut entries = tokio::fs::read_dir(&teams_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if should_remove(&path, args.older_than).await? {
                let size = dir_size(&path).await?;
                if args.dry_run {
                    println!("Would remove: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
                } else {
                    tokio::fs::remove_dir_all(&path).await?;
                    info!(path = %path.display(), "Removed team state");
                    println!("✓ Removed: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
                }
                removed += 1;
                freed += size;
            }
        }
    }

    // Scan autopilot states
    let autopilot_dir = state_dir.join("autopilot");
    if autopilot_dir.exists() {
        let mut entries = tokio::fs::read_dir(&autopilot_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if should_remove(&path, args.older_than).await? {
                let size = dir_size(&path).await?;
                if args.dry_run {
                    println!("Would remove: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
                } else {
                    tokio::fs::remove_dir_all(&path).await?;
                    info!(path = %path.display(), "Removed autopilot state");
                    println!("✓ Removed: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
                }
                removed += 1;
                freed += size;
            }
        }
    }

    // Scan ralph states
    let ralph_dir = state_dir.join("ralph");
    if ralph_dir.exists() {
        let mut entries = tokio::fs::read_dir(&ralph_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if should_remove(&path, args.older_than).await? {
                let size = dir_size(&path).await?;
                if args.dry_run {
                    println!("Would remove: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
                } else {
                    tokio::fs::remove_dir_all(&path).await?;
                    info!(path = %path.display(), "Removed ralph state");
                    println!("✓ Removed: {} ({:.1} MB)", path.display(), size as f64 / 1_048_576.0);
                }
                removed += 1;
                freed += size;
            }
        }
    }

    println!();
    if args.dry_run {
        println!("Would remove {removed} state directories ({:.1} MB)", freed as f64 / 1_048_576.0);
    } else {
        println!("Removed {removed} state directories ({:.1} MB freed)", freed as f64 / 1_048_576.0);
    }

    Ok(())
}

async fn should_remove(path: &Path, older_than: Option<u64>) -> Result<bool> {
    if let Some(days) = older_than {
        let metadata = tokio::fs::metadata(path).await?;
        let modified = metadata.modified()?;
        let age = modified.elapsed()?.as_secs() / 86400;
        return Ok(age >= days);
    }
    Ok(true)
}

async fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total += metadata.len();
            }
        }
    }

    Ok(total)
}
