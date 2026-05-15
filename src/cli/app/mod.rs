use clap::{Parser, Subcommand, ValueEnum};

use super::kimi_native_cmd;
use super::{
    ask, autopilot, backup, cleanup, config_cmd, cost_cmd, doctor, goal, hud, logs, marketplace,
    proof_cmd, ralph, run_cmd, skill, state, team, ultrawork,
};

mod run;
mod setup;
mod update;

pub use run::run;

/// Oh My Kimi — Multi-agent orchestration for Kimi CLI
#[derive(Parser, Debug)]
#[command(name = "omk")]
#[command(about = "Scheduler-backed team orchestration and Kimi asset tooling")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub(super) struct Omk {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
pub(super) enum Commands {
    /// Team orchestration (Wire scheduler runtime)
    #[command(visible_alias = "t")]
    Team(team::Args),
    /// Run autonomous execution (single lead agent)
    #[command(visible_alias = "ap")]
    Autopilot(autopilot::Args),
    /// Persistent mode with verify/fix loops
    #[command(visible_alias = "r")]
    Ralph(ralph::Args),
    /// Ask a provider advisor
    Ask(ask::Args),
    /// HUD / statusline
    Hud(hud::Args),
    /// Setup OMK (install hooks, skills, config)
    Setup,
    /// Update OMK
    Update(UpdateArgs),
    /// Run MCP server
    McpServer,
    /// Generate shell completions
    Completions(CompletionsArgs),
    /// View logs
    Logs(logs::Args),
    /// Generate man page
    Man,
    /// Diagnose environment and dependencies
    Doctor(doctor::Args),
    /// Clean up old state files
    Cleanup(cleanup::Args),
    /// Manage configuration
    Config(config_cmd::Args),
    /// Backup and restore state
    Backup(backup::Args),
    /// Export/import state
    State(state::Args),
    /// Manage skills
    #[command(visible_alias = "s")]
    Skill(skill::Args),
    /// Browse skill marketplace
    #[command(visible_alias = "m")]
    Marketplace(marketplace::Args),
    /// Cost tracking and estimation
    Cost(cost_cmd::Args),
    /// Goal runtime (durable autonomous controller scaffold)
    Goal(goal::Args),
    /// Parallel burst execution without a team
    #[command(visible_alias = "uw")]
    Ultrawork(ultrawork::Args),
    /// Kimi asset commands (sync/install/doctor + listing/rollback surfaces)
    #[command(name = "kimi", visible_alias = "k")]
    KimiNative(kimi_native_cmd::KimiNativeArgs),
    /// Inspect run event timelines
    Run(run_cmd::Args),
    /// Generate and view proof reports
    Proof(proof_cmd::Args),
    /// Show version information
    Version,
}

#[derive(Parser, Debug)]
pub(super) struct UpdateArgs {
    /// Check for updates without installing
    #[arg(long)]
    check: bool,
}

#[derive(Parser, Debug)]
pub(super) struct CompletionsArgs {
    #[arg(value_enum)]
    shell: ShellArg,
}

#[derive(Clone, Debug, ValueEnum)]
pub(super) enum ShellArg {
    Bash,
    Zsh,
    Fish,
    Elvish,
    PowerShell,
}
