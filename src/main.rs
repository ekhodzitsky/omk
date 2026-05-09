#![allow(dead_code)]

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use tracing::info;

mod agents;
mod cli;
mod cost;
mod error;
mod kimi_native;
mod marketplace;
mod mcp;
mod notifications;
mod runtime;
mod skills;
mod vis;
mod wire;

use cli::kimi_native_cmd;
use cli::{
    ask, autopilot, backup, cleanup, config_cmd, cost_cmd, doctor, hud, logs, proof_cmd, ralph,
    run_cmd, skill, state, team, ultrawork,
};

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
    Marketplace(cli::marketplace::Args),
    /// Cost tracking and estimation
    Cost(cost_cmd::Args),
    /// Parallel burst execution without a team
    #[command(visible_alias = "uw")]
    Ultrawork(ultrawork::Args),
    /// Kimi-native integration (sync, doctor, install)
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
struct UpdateArgs {
    /// Check for updates without installing
    #[arg(long)]
    check: bool,
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

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if omk.verbose {
            tracing_subscriber::EnvFilter::new("debug")
        } else {
            tracing_subscriber::EnvFilter::new("info")
        }
    });

    // File logging with daily rotation
    let log_dir = crate::runtime::config::state_dir().join("logs");
    tokio::fs::create_dir_all(&log_dir).await?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "omk.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let stderr_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    info!("omk starting");

    match omk.command {
        Commands::Team(args) => team::run(args).await,
        Commands::Autopilot(args) => autopilot::run(args).await,
        Commands::Ralph(args) => ralph::run(args).await,
        Commands::Ask(args) => ask::run(args).await,
        Commands::Hud(args) => hud::run(args).await,
        Commands::Setup => run_setup().await,
        Commands::Update(args) => run_update(args).await,
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
        Commands::Doctor(args) => doctor::run(args).await,
        Commands::Cleanup(args) => cleanup::run(args).await,
        Commands::Logs(args) => logs::run(args).await,
        Commands::Cost(args) => cost_cmd::run(args).await,
        Commands::Config(args) => config_cmd::run(args).await,
        Commands::Backup(args) => backup::run(args).await,
        Commands::State(args) => state::run(args).await,
        Commands::Skill(args) => skill::run(args).await,
        Commands::Marketplace(args) => cli::marketplace::run(args).await,
        Commands::Ultrawork(args) => ultrawork::run(args).await,
        Commands::KimiNative(args) => kimi_native_cmd::run(args).await,
        Commands::Run(args) => run_cmd::run(args).await,
        Commands::Proof(args) => proof_cmd::run(args).await,
        Commands::Version => {
            println!("omk {}", env!("CARGO_PKG_VERSION"));
            println!("  Repository: {}", env!("CARGO_PKG_REPOSITORY"));
            println!("  Rust: {}", rustc_version_runtime::version());
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

    // Write default AGENTS.md in current project if .omk/ exists or is created
    let project_omk = std::env::current_dir()?.join(".omk");
    tokio::fs::create_dir_all(&project_omk).await.ok();
    let agents_path = project_omk.join("AGENTS.md");
    if !agents_path.exists() {
        tokio::fs::write(&agents_path, crate::agents::runtime::default_agents_md()).await?;
        println!("✓ Created {}", agents_path.display());
    }

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

async fn run_update(args: UpdateArgs) -> Result<()> {
    use tokio::process::Command;

    let current = env!("CARGO_PKG_VERSION");
    println!("Current version: {current}");

    // Detect platform
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let target = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => {
            anyhow::bail!("Unsupported platform: {os} {arch}");
        }
    };

    // Fetch latest release tag
    println!("Checking for latest release...");
    let latest = match Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "Accept: application/vnd.github+json",
            "https://api.github.com/repos/ekhodzitsky/oh-my-kimi/releases/latest",
        ])
        .output()
        .await
    {
        Ok(out) if out.status.success() => {
            let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
            json["tag_name"].as_str().unwrap_or("").to_string()
        }
        _ => {
            anyhow::bail!("Failed to check for updates. Are you online?");
        }
    };

    if latest.is_empty() {
        anyhow::bail!("Could not determine latest version");
    }

    let latest_version = latest.trim_start_matches('v');
    println!("Latest version: {latest_version}");

    if latest_version == current {
        println!("✓ You are already on the latest version ({current}).");
        return Ok(());
    }

    if args.check {
        println!("Update available: {current} → {latest_version}");
        println!("Run `omk update` to install.");
        return Ok(());
    }

    let url = format!(
        "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/{latest}/omk-{latest_version}-{target}.tar.gz"
    );

    println!("Downloading {url}...");

    let tmp_dir = tempfile::tempdir()?;
    let tar_path = tmp_dir.path().join("omk.tar.gz");

    let download = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&tar_path)
        .arg(&url)
        .status()
        .await?;

    if !download.success() {
        anyhow::bail!("Download failed. Prebuilt binary may not be available for {target}.");
    }

    println!("Extracting...");
    let extract = Command::new("tar")
        .args(["-xzf"])
        .arg(&tar_path)
        .arg("-C")
        .arg(tmp_dir.path())
        .status()
        .await?;

    if !extract.success() {
        anyhow::bail!("Failed to extract archive");
    }

    // Find the binary
    let new_binary = tmp_dir
        .path()
        .join(format!("omk-{latest_version}-{target}"))
        .join("omk");
    if !new_binary.exists() {
        // Fallback: binary might be at top level
        let fallback = tmp_dir.path().join("omk");
        if fallback.exists() {
            tokio::fs::copy(&fallback, &new_binary).await?;
        } else {
            anyhow::bail!("Could not find omk binary in downloaded archive");
        }
    }

    // Replace current binary
    let current_exe = std::env::current_exe()?;
    println!("Replacing {}...", current_exe.display());

    // On Unix, we can atomically replace
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&new_binary).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&new_binary, perms).await?;
    }

    tokio::fs::copy(&new_binary, &current_exe).await?;

    println!("✓ Updated to {latest_version}");
    println!("  Binary: {}", current_exe.display());

    // Update completions
    println!("Updating shell completions...");
    let _ = Command::new(&current_exe)
        .args(["completions", "bash"])
        .output()
        .await;
    let _ = Command::new(&current_exe)
        .args(["completions", "zsh"])
        .output()
        .await;
    let _ = Command::new(&current_exe)
        .args(["completions", "fish"])
        .output()
        .await;

    println!("Run `omk doctor` to verify the installation.");
    Ok(())
}
