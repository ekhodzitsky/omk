use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::cli::{
    ask, autopilot, backup, cleanup, config_cmd, cost_cmd, doctor, goal, hud, logs, marketplace,
    proof_cmd, ralph, run_cmd, skill, state, team, ultrawork,
};
use crate::cli::kimi_native_cmd;
use super::{setup, update, Commands, Omk, ShellArg};

pub async fn run() -> Result<()> {
    let cancel = CancellationToken::new();
    let mut run_fut = std::pin::pin!(run_with_cancel(cancel.clone()));

    tokio::select! {
        res = &mut run_fut => res,
        sig = wait_for_signal() => {
            if let Err(e) = sig {
                error!("failed to install signal handler: {e}");
            }
            eprintln!("\nReceived shutdown signal, cancelling...");
            cancel.cancel();
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                &mut run_fut,
            ).await;
            Ok(())
        }
    }
}

async fn run_with_cancel(cancel: CancellationToken) -> Result<()> {
    let omk = Omk::parse();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if omk.verbose {
            tracing_subscriber::EnvFilter::new("debug")
        } else {
            tracing_subscriber::EnvFilter::new("info")
        }
    });

    let state_dir = crate::runtime::config::state_dir();
    crate::runtime::config::ensure_private_dir(&state_dir).await?;
    let log_dir = state_dir.join("logs");
    crate::runtime::config::ensure_private_dir(&log_dir).await?;
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
        Commands::Team(args) => team::run(args, cancel.clone()).await,
        Commands::Autopilot(args) => autopilot::run(args, cancel.clone()).await,
        Commands::Ralph(args) => ralph::run(args, cancel.clone()).await,
        Commands::Ask(args) => ask::run(args).await,
        Commands::Hud(args) => hud::run(args).await,
        Commands::Setup => setup::run_setup().await,
        Commands::Update(args) => update::run_update(args).await,
        Commands::McpServer => crate::mcp::run_mcp_server().await,
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
        Commands::Goal(args) => goal::run(args).await,
        Commands::Config(args) => config_cmd::run(args).await,
        Commands::Backup(args) => backup::run(args).await,
        Commands::State(args) => state::run(args).await,
        Commands::Skill(args) => skill::run(args).await,
        Commands::Marketplace(args) => marketplace::run(args).await,
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

#[cfg(unix)]
async fn wait_for_signal() -> std::io::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => Ok(()),
        _ = sigterm.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}
