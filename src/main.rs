use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
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
    /// Run MCP server
    McpServer,
    /// Generate shell completions
    Completions(CompletionsArgs),
    /// Generate man page
    Man,
}

#[derive(Parser, Debug)]
struct CompletionsArgs {
    #[arg(value_enum)]
    shell: ShellArg,
}

#[derive(Clone, Debug, ValueEnum)]
enum ShellArg {
    Bash,
    Zsh,
    Fish,
    Elvish,
    PowerShell,
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
        Commands::McpServer => mcp::run_mcp_server().await,
        Commands::Completions(args) => {
            let shell = match args.shell {
                ShellArg::Bash => Shell::Bash,
                ShellArg::Zsh => Shell::Zsh,
                ShellArg::Fish => Shell::Fish,
                ShellArg::Elvish => Shell::Elvish,
                ShellArg::PowerShell => Shell::PowerShell,
            };
            let mut cmd = Omk::command();
            generate(shell, &mut cmd, "omk", &mut std::io::stdout());
            Ok(())
        }
        Commands::Man => {
            let cmd = Omk::command();
            let man = clap_mangen::Man::new(cmd);
            man.render(&mut std::io::stdout())?;
            Ok(())
        }
    }
}

async fn run_setup() -> Result<()> {
    info!("Running omk setup");

    crate::runtime::config::ensure_dirs().await?;

    let config_dir = crate::runtime::config::config_dir();
    let state_dir = crate::runtime::config::state_dir();
    let data_dir = crate::runtime::config::data_dir();

    // Write default config if missing
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = r#"# OMK Configuration
# See https://github.com/ekhodzitsky/oh-my-kimi for docs

# Default number of workers for team mode
default_team_size = 2

# Enable YOLO (auto-approve) mode by default
default_yolo = false

# Path to Kimi CLI binary (leave empty for auto-detect)
# kimi_binary = "/usr/local/bin/kimi"

# Additional skill directories
# extra_skill_dirs = ["~/.omk/skills"]

# Enable metrics collection
enable_metrics = true
"#;
        tokio::fs::write(&config_path, default_config).await?;
    }

    // Install bundled skills to data dir
    let skills_dir = data_dir.join("skills");
    tokio::fs::create_dir_all(&skills_dir).await?;
    // TODO: copy bundled skills from repo to skills_dir

    println!("✓ omk setup complete");
    println!("  Config: {}", config_dir.display());
    println!("  State:  {}", state_dir.display());
    println!("  Data:   {}", data_dir.display());
    println!();
    println!("Next steps:");
    println!("  1. Ensure 'kimi' CLI is installed and authenticated");
    println!("  2. Run 'omk team spawn 2:coder \"fix TypeScript errors\"' to try team mode");

    Ok(())
}

async fn run_update() -> Result<()> {
    warn!("omk update: not yet implemented");
    println!("omk update is not yet implemented.");
    println!("Please update via your package manager (cargo install omk).");
    Ok(())
}
