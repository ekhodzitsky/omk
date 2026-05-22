use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::kimi_native::role_packs::RolePack;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[command(subcommand)]
    pub(crate) command: TeamCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum TeamCommands {
    /// Run a scheduler-backed team workflow
    Run(RunArgs),
    /// List all active teams
    List,
    /// Check team status
    Status(StatusArgs),
    /// Rename a team
    Rename(RenameArgs),
    /// Export a team state to JSON
    Export(ExportArgs),
    /// Import a team state from JSON
    Import(ImportArgs),
    /// Shutdown a team
    Shutdown(ShutdownArgs),
    /// Run watchdog health check on a team
    Health(HealthArgs),
    /// Clean up old team state directories
    Cleanup(CleanupArgs),
    /// List available role packs
    Roles,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct RunArgs {
    #[arg(value_name = "N:ROLE")]
    pub spec: String,

    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub task: Vec<String>,

    #[arg(short, long)]
    pub name: Option<String>,

    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    #[arg(long)]
    pub no_ralph: bool,

    /// Select specific verification gates to run (default: all)
    #[arg(long, value_delimiter = ',')]
    pub gate: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct StatusArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct RenameArgs {
    #[arg(value_name = "OLD_NAME")]
    pub old_name: String,

    #[arg(value_name = "NEW_NAME")]
    pub new_name: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct ExportArgs {
    #[arg(value_name = "NAME")]
    pub name: String,

    #[arg(short, long, default_value = "team-export.json")]
    pub output: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct ImportArgs {
    #[arg(value_name = "FILE")]
    pub file: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct ShutdownArgs {
    #[arg(value_name = "NAME")]
    pub name: String,

    #[arg(long)]
    pub force: bool,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct HealthArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct CleanupArgs {
    /// Remove team states older than N days
    #[arg(long, default_value = "7")]
    pub older_than: u64,

    /// Dry run: show what would be removed
    #[arg(long)]
    pub dry_run: bool,

    /// Remove all team states (ignore age filter)
    #[arg(long)]
    pub all: bool,
}

pub(crate) fn parse_spec(spec: &str) -> Result<(usize, String)> {
    if let Some(resolved) = resolve_role_alias(spec) {
        return Ok(resolved);
    }

    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid spec '{}'. Expected format: N:role (e.g. 3:coder)",
            spec
        );
    }
    let count: usize = parts[0]
        .parse()
        .with_context(|| format!("Invalid worker count '{}'", parts[0]))?;
    if count == 0 || count > 16 {
        anyhow::bail!("Worker count must be between 1 and 16");
    }
    Ok((count, parts[1].to_string()))
}

fn resolve_role_alias(alias: &str) -> Option<(usize, String)> {
    match alias {
        "team" => Some((3, "executor".to_string())),
        _ => RolePack::find(alias).map(|p| (p.suggested_worker_count, p.id)),
    }
}

pub(crate) fn roles() -> Result<()> {
    println!("Role Packs");
    println!("{}", "━".repeat(40));
    for pack in RolePack::all() {
        println!(
            "{:<12} {:<2} {}",
            pack.id, pack.suggested_worker_count, pack.description
        );
    }
    Ok(())
}
