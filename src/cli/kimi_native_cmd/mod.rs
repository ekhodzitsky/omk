use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod agents;
mod doctor;
mod hooks;
mod install;
mod rollback;
mod skills;
mod sync;

#[derive(Parser, Debug)]
pub struct KimiNativeArgs {
    #[command(subcommand)]
    pub command: KimiNativeCommands,
}

#[derive(Subcommand, Debug)]
pub enum KimiNativeCommands {
    /// Sync OMK assets for current Kimi surfaces (project + user scope)
    Sync {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(short, long, help = "Force overwrite even if files exist")]
        force: bool,
        #[arg(long, help = "Show what would be done without making changes")]
        dry_run: bool,
    },
    /// Validate Kimi-native configuration and assets
    Doctor {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Output results as JSON")]
        json: bool,
    },
    /// Install OMK role assets into the current project's Kimi workspace
    Install {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Show what would be installed without making changes")]
        dry_run: bool,
    },
    /// List bundled OMK role agent templates
    Agents,
    /// List bundled OMK project hook templates
    Hooks,
    /// List discovered OMK skills in the local data directory
    Skills,
    /// Rollback OMK-installed Kimi assets from .kimi/
    Rollback {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Show what would be removed without making changes")]
        dry_run: bool,
    },
}

pub(crate) async fn run(args: KimiNativeArgs) -> Result<()> {
    match args.command {
        KimiNativeCommands::Sync {
            dir,
            force,
            dry_run,
        } => sync::cmd_sync(&dir, force, dry_run).await,
        KimiNativeCommands::Doctor { dir, json } => doctor::cmd_doctor(&dir, json).await,
        KimiNativeCommands::Install { dir, dry_run } => install::cmd_install(&dir, dry_run).await,
        KimiNativeCommands::Agents => agents::cmd_agents(),
        KimiNativeCommands::Hooks => hooks::cmd_hooks(),
        KimiNativeCommands::Skills => skills::cmd_skills().await,
        KimiNativeCommands::Rollback { dir, dry_run } => {
            rollback::cmd_rollback(&dir, dry_run).await
        }
    }
}
