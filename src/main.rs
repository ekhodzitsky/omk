use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, warn};

mod cli;
mod mcp;
mod runtime;
mod skills;
mod vis;

use cli::{ask, autopilot, hud, ralph, team};

/// Oh My Kimi — Multi-agent orchestration for Kimi CLI
#[derive(Parser, Debug)]
#[command(name = "omk")]
#[command(about = "Multi-agent orchestration for Kimi CLI")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Omk {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Spawn a team of Kimi agents in tmux
    Team(team::Args),
    /// Run autonomous execution (single lead agent)
    Autopilot(autopilot::Args),
    /// Persistent mode with verify/fix loops
    Ralph(ralph::Args),
    /// Ask a provider advisor
    Ask(ask::Args),
    /// HUD / statusline
    Hud(hud::Args),
    /// Setup OMK (install hooks, skills, config)
    Setup,
    /// Update OMK
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    let omk = Omk::parse();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    if omk.verbose {
                        tracing_subscriber::EnvFilter::new("debug")
                    } else {
                        tracing_subscriber::EnvFilter::new("info")
                    }
                }),
        )
        .with_writer(std::io::stderr)
        .finish();
    
    let _guard = tracing::subscriber::set_default(subscriber);

    info!("omk starting");

    match omk.command {
        Commands::Team(args) => team::run(args).await,
        Commands::Autopilot(args) => autopilot::run(args).await,
        Commands::Ralph(args) => ralph::run(args).await,
        Commands::Ask(args) => ask::run(args).await,
        Commands::Hud(args) => hud::run(args).await,
        Commands::Setup => run_setup().await,
        Commands::Update => run_update().await,
    }
}

async fn run_setup() -> Result<()> {
    info!("Running omk setup");
    
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("omk");
    
    let omk_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".omk");

    tokio::fs::create_dir_all(&config_dir).await?;
    tokio::fs::create_dir_all(&omk_dir).await?;
    tokio::fs::create_dir_all(omk_dir.join("skills")).await?;
    tokio::fs::create_dir_all(omk_dir.join("state")).await?;
    tokio::fs::create_dir_all(omk_dir.join("artifacts")).await?;

    // TODO: Install bundled skills to ~/.omk/skills/
    // TODO: Install Kimi CLI hooks if requested
    // TODO: Write default config.toml

    println!("✓ omk setup complete");
    println!("  Config: {}", config_dir.display());
    println!("  State:  {}", omk_dir.display());
    println!();
    println!("Next steps:");
    println!("  1. Ensure 'kimi' CLI is installed and authenticated");
    println!("  2. Run 'omk team 2:coder \"fix TypeScript errors\"' to try team mode");

    Ok(())
}

async fn run_update() -> Result<()> {
    warn!("omk update: not yet implemented");
    println!("omk update is not yet implemented.");
    println!("Please update via your package manager (cargo install omk).");
    Ok(())
}
