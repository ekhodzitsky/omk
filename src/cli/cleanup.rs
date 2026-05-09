use anyhow::Result;
use clap::Parser;
use std::path::Path;
use tracing::info;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// Remove all state (teams, autopilot, ralph)
    #[arg(long)]
    all: bool,

    /// Remove state older than N days
    #[arg(long, value_name = "DAYS")]
    older_than: Option<u64>,

    /// Remove old artifacts (ask outputs, logs)
    #[arg(long)]
    artifacts: bool,

    /// Dry run: show what would be removed
    #[arg(long)]
    dry_run: bool,

    /// Clean only team states
    #[arg(long)]
    teams: bool,
}

pub(crate) async fn cleanup_team_states(
    teams_dir: &Path,
    older_than: Option<u64>,
    dry_run: bool,
) -> Result<(usize, u64)> {
    let mut removed = 0;
    let mut freed: u64 = 0;

    if !teams_dir.exists() {
        return Ok((removed, freed));
    }

    let mut entries = tokio::fs::read_dir(teams_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if should_remove(&path, older_than).await? {
            let size = dir_size(&path).await?;
            if dry_run {
                println!(
                    "Would remove: {} ({:.1} MB)",
                    path.display(),
                    size as f64 / 1_048_576.0
                );
            } else {
                tokio::fs::remove_dir_all(&path).await?;
                info!(path = %path.display(), "Removed team state");
                println!(
                    "✓ Removed: {} ({:.1} MB)",
                    path.display(),
                    size as f64 / 1_048_576.0
                );
            }
            removed += 1;
            freed += size;
        }
    }

    Ok((removed, freed))
}

pub(crate) async fn run(args: Args) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir();

    if args.teams {
        let teams_dir = state_dir.join("team");
        let (removed, freed) =
            cleanup_team_states(&teams_dir, args.older_than, args.dry_run).await?;
        println!();
        if args.dry_run {
            println!(
                "Would remove {removed} team state directories ({:.1} MB)",
                freed as f64 / 1_048_576.0
            );
        } else {
            println!(
                "Removed {removed} team state directories ({:.1} MB freed)",
                freed as f64 / 1_048_576.0
            );
        }
        return Ok(());
    }

    if args.artifacts {
        let mut removed = 0;
        let mut freed: u64 = 0;

        let artifacts_dir = crate::runtime::config::data_dir().join("artifacts");
        if artifacts_dir.exists() {
            let mut entries = tokio::fs::read_dir(&artifacts_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if should_remove(&path, args.older_than).await? {
                    let size = dir_size(&path).await?;
                    if args.dry_run {
                        println!(
                            "Would remove: {} ({:.1} MB)",
                            path.display(),
                            size as f64 / 1_048_576.0
                        );
                    } else {
                        tokio::fs::remove_dir_all(&path).await?;
                        info!(path = %path.display(), "Removed artifacts");
                        println!(
                            "✓ Removed: {} ({:.1} MB)",
                            path.display(),
                            size as f64 / 1_048_576.0
                        );
                    }
                    removed += 1;
                    freed += size;
                }
            }
        }

        let logs_dir = crate::runtime::config::state_dir().join("logs");
        if logs_dir.exists() {
            let mut entries = tokio::fs::read_dir(&logs_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if should_remove(&path, args.older_than).await? {
                    let size = entry.metadata().await?.len();
                    if args.dry_run {
                        println!(
                            "Would remove: {} ({:.1} MB)",
                            path.display(),
                            size as f64 / 1_048_576.0
                        );
                    } else {
                        tokio::fs::remove_file(&path).await?;
                        info!(path = %path.display(), "Removed log file");
                        println!(
                            "✓ Removed: {} ({:.1} MB)",
                            path.display(),
                            size as f64 / 1_048_576.0
                        );
                    }
                    removed += 1;
                    freed += size;
                }
            }
        }

        println!();
        if args.dry_run {
            println!(
                "Would remove {removed} artifact directories/log files ({:.1} MB)",
                freed as f64 / 1_048_576.0
            );
        } else {
            println!(
                "Removed {removed} artifact directories/log files ({:.1} MB freed)",
                freed as f64 / 1_048_576.0
            );
        }
        return Ok(());
    }

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
        let (r, f) = cleanup_team_states(&teams_dir, args.older_than, args.dry_run).await?;
        removed += r;
        freed += f;
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
                    println!(
                        "Would remove: {} ({:.1} MB)",
                        path.display(),
                        size as f64 / 1_048_576.0
                    );
                } else {
                    tokio::fs::remove_dir_all(&path).await?;
                    info!(path = %path.display(), "Removed autopilot state");
                    println!(
                        "✓ Removed: {} ({:.1} MB)",
                        path.display(),
                        size as f64 / 1_048_576.0
                    );
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
                    println!(
                        "Would remove: {} ({:.1} MB)",
                        path.display(),
                        size as f64 / 1_048_576.0
                    );
                } else {
                    tokio::fs::remove_dir_all(&path).await?;
                    info!(path = %path.display(), "Removed ralph state");
                    println!(
                        "✓ Removed: {} ({:.1} MB)",
                        path.display(),
                        size as f64 / 1_048_576.0
                    );
                }
                removed += 1;
                freed += size;
            }
        }
    }

    println!();
    if args.dry_run {
        println!(
            "Would remove {removed} state directories ({:.1} MB)",
            freed as f64 / 1_048_576.0
        );
    } else {
        println!(
            "Removed {removed} state directories ({:.1} MB freed)",
            freed as f64 / 1_048_576.0
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cleanup_team_states_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let teams_dir = dir.path().join("team");
        tokio::fs::create_dir_all(&teams_dir).await.unwrap();

        for name in ["team-a", "team-b"] {
            let team_dir = teams_dir.join(name);
            tokio::fs::create_dir_all(&team_dir).await.unwrap();
            tokio::fs::write(team_dir.join("team-state.json"), r#"{"name":"test"}"#)
                .await
                .unwrap();
        }

        let (removed, freed) = cleanup_team_states(&teams_dir, None, true).await.unwrap();
        assert_eq!(removed, 2);
        assert!(freed > 0);

        assert!(teams_dir.join("team-a").exists());
        assert!(teams_dir.join("team-b").exists());
    }

    #[tokio::test]
    async fn test_cleanup_team_states_removes_all() {
        let dir = tempfile::tempdir().unwrap();
        let teams_dir = dir.path().join("team");
        tokio::fs::create_dir_all(&teams_dir).await.unwrap();

        for name in ["team-a", "team-b"] {
            let team_dir = teams_dir.join(name);
            tokio::fs::create_dir_all(&team_dir).await.unwrap();
            tokio::fs::write(team_dir.join("team-state.json"), r#"{"name":"test"}"#)
                .await
                .unwrap();
        }

        let (removed, freed) = cleanup_team_states(&teams_dir, None, false).await.unwrap();
        assert_eq!(removed, 2);
        assert!(freed > 0);

        assert!(!teams_dir.join("team-a").exists());
        assert!(!teams_dir.join("team-b").exists());
    }

    #[tokio::test]
    async fn test_cleanup_team_states_age_filter_zero() {
        let dir = tempfile::tempdir().unwrap();
        let teams_dir = dir.path().join("team");
        tokio::fs::create_dir_all(&teams_dir).await.unwrap();

        for name in ["team-a", "team-b"] {
            let team_dir = teams_dir.join(name);
            tokio::fs::create_dir_all(&team_dir).await.unwrap();
            tokio::fs::write(team_dir.join("team-state.json"), r#"{"name":"test"}"#)
                .await
                .unwrap();
        }

        // older_than = 0 means everything is old enough to remove
        let (removed, _freed) = cleanup_team_states(&teams_dir, Some(0), false)
            .await
            .unwrap();
        assert_eq!(removed, 2);
    }

    #[tokio::test]
    async fn test_cleanup_team_states_age_filter_future() {
        let dir = tempfile::tempdir().unwrap();
        let teams_dir = dir.path().join("team");
        tokio::fs::create_dir_all(&teams_dir).await.unwrap();

        for name in ["team-a", "team-b"] {
            let team_dir = teams_dir.join(name);
            tokio::fs::create_dir_all(&team_dir).await.unwrap();
            tokio::fs::write(team_dir.join("team-state.json"), r#"{"name":"test"}"#)
                .await
                .unwrap();
        }

        // older_than = 100000 means nothing is old enough to remove
        let (removed, _freed) = cleanup_team_states(&teams_dir, Some(100000), false)
            .await
            .unwrap();
        assert_eq!(removed, 0);

        assert!(teams_dir.join("team-a").exists());
        assert!(teams_dir.join("team-b").exists());
    }
}
