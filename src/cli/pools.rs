use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::runtime::scheduler::pool::PoolManager;

#[derive(Parser, Debug)]
#[command(about = "Manage agent pools and resource limits")]
pub struct Args {
    #[command(subcommand)]
    pub command: PoolsCommands,
}

#[derive(Subcommand, Debug)]
pub enum PoolsCommands {
    /// Show pool status (active, queued, disk usage)
    Status {
        /// Filter to a specific pool
        #[arg(short, long)]
        pool: Option<String>,
    },
    /// Clean up completed worktrees and free disk budget
    Cleanup {
        /// Target pool (all pools if omitted)
        #[arg(short, long)]
        pool: Option<String>,
        /// Force removal even if worktree retention policy has not expired
        #[arg(long)]
        force: bool,
    },
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        PoolsCommands::Status { pool } => cmd_status(pool).await,
        PoolsCommands::Cleanup { pool, force } => cmd_cleanup(pool, force),
    }
}

async fn cmd_status(pool_filter: Option<String>) -> Result<()> {
    // For now we show a basic status from a default PoolManager.
    // In a full integration the manager would be retrieved from runtime state.
    let manager = PoolManager::new(std::collections::HashMap::new());
    let statuses = manager.all_statuses().await;

    for status in statuses {
        if let Some(ref filter) = pool_filter {
            if &status.name != filter {
                continue;
            }
        }
        let disk_gb = status.disk_usage_bytes as f64 / 1_000_000_000.0;
        let max_disk_str = status
            .max_disk_gb
            .map(|g| format!("{:.1}GB", g as f64))
            .unwrap_or_else(|| "unlimited".to_string());
        println!(
            "{}: {}/{} active, {} queued, {:.2}GB/{} disk",
            status.name,
            status.active_count,
            status.max_workers,
            status.queued_count,
            disk_gb,
            max_disk_str,
        );
    }
    Ok(())
}

fn cmd_cleanup(pool_filter: Option<String>, force: bool) -> Result<()> {
    // Placeholder: real cleanup would scan worktrees, measure sizes,
    // call PoolManager::update_disk_usage, and remove directories.
    if force {
        println!(
            "Force cleanup requested for pools: {:?}",
            pool_filter.as_deref().unwrap_or("all")
        );
    } else {
        println!(
            "Cleanup requested for pools: {:?}",
            pool_filter.as_deref().unwrap_or("all")
        );
    }
    Ok(())
}
