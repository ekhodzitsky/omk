use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::{mcp_cmd, setup, update, Commands, Omk, ShellArg};
use crate::cli::kimi_native_cmd;
use crate::cli::{
    ask, autopilot, backup, cleanup, config_cmd, cost_cmd, doctor, gates, goal, hud, logs,
    marketplace, pools, proof_cmd, ralph, run_cmd, skill, state, team, ultrawork,
};

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
            // Flush circuit breaker state before exit.
            if let Err(e) = flush_circuit_breakers().await {
                warn!(error = %e, "Failed to flush circuit breakers on shutdown");
            }
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

    #[cfg(feature = "tokio-console")]
    {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .with(console_subscriber::spawn())
            .init();
    }
    #[cfg(not(feature = "tokio-console"))]
    {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
    }

    info!("omk starting");

    // Initialize circuit breaker registry with the central database.
    if let Err(e) = init_circuit_breaker_registry().await {
        warn!(error = %e, "Failed to initialize circuit breaker registry with DB; running in-memory only");
    }

    match omk.command {
        Some(Commands::Chat(args)) => crate::cli::chat::run::run_chat_async(args).await,
        None => {
            crate::cli::chat::run::run_chat_async(crate::cli::chat::run::ChatArgs::default()).await
        }
        Some(Commands::Team(args)) => team::run(args, cancel.clone()).await,
        Some(Commands::Autopilot(args)) => autopilot::run(args, cancel.clone()).await,
        Some(Commands::Ralph(args)) => ralph::run(args, cancel.clone()).await,
        Some(Commands::Ask(args)) => ask::run(args).await,
        Some(Commands::Hud(args)) => hud::run(args).await,
        Some(Commands::Setup) => setup::run_setup().await,
        Some(Commands::Update(args)) => update::run_update(args).await,
        Some(Commands::Mcp(args)) => mcp_cmd::run(args).await,
        Some(Commands::McpServer) => crate::mcp::run_mcp_server().await,
        Some(Commands::Completions(args)) => {
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
        Some(Commands::Man) => {
            let cmd = Omk::command();
            let man = clap_mangen::Man::new(cmd);
            man.render(&mut std::io::stdout())?;
            Ok(())
        }
        Some(Commands::Doctor(args)) => doctor::run(args).await,
        Some(Commands::Cleanup(args)) => cleanup::run(args).await,
        Some(Commands::Logs(args)) => logs::run(args).await,
        Some(Commands::Cost(args)) => cost_cmd::run(args).await,
        Some(Commands::Goal(args)) => goal::run(args).await,
        Some(Commands::Config(args)) => config_cmd::run(args).await,
        Some(Commands::Backup(args)) => backup::run(args).await,
        Some(Commands::State(args)) => state::run(args).await,
        Some(Commands::Skill(args)) => skill::run(args).await,
        Some(Commands::Marketplace(args)) => marketplace::run(args).await,
        Some(Commands::Ultrawork(args)) => ultrawork::run(args).await,
        Some(Commands::Pools(args)) => pools::run(args).await,
        Some(Commands::KimiNative(args)) => kimi_native_cmd::run(args).await,
        Some(Commands::Run(args)) => run_cmd::run(args).await,
        Some(Commands::Proof(args)) => proof_cmd::run(args).await,
        Some(Commands::Gates(args)) => gates::run(args).await,
        Some(Commands::Version) => {
            println!("omk {}", env!("CARGO_PKG_VERSION"));
            println!("  Repository: {}", env!("CARGO_PKG_REPOSITORY"));
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

async fn init_circuit_breaker_registry() -> anyhow::Result<()> {
    let db_path = crate::runtime::config::omk_state_dir().join("omk.db");
    let db = crate::runtime::db::handle::DbHandle::open(&db_path).await?;
    let repo = db.circuit_breaker_repo();
    let registry = crate::runtime::gates::circuit_breaker::CircuitBreakerRegistry::with_repo(repo);
    registry.load_from_db().await?;
    if crate::runtime::gates::circuit_breaker::init_global_registry(registry).is_err() {
        anyhow::bail!("Global circuit breaker registry already initialized");
    }
    Ok(())
}

async fn flush_circuit_breakers() -> anyhow::Result<()> {
    let registry = crate::runtime::gates::circuit_breaker::global_registry();
    registry.flush().await?;
    Ok(())
}
