use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use super::kimi_native_cmd;
use super::{
    ask, autopilot, backup, cleanup, config_cmd, cost_cmd, doctor, goal, hud, logs, marketplace,
    proof_cmd, ralph, run_cmd, skill, state, team, ultrawork,
};

/// Oh My Kimi — Multi-agent orchestration for Kimi CLI
#[derive(Parser, Debug)]
#[command(name = "omk")]
#[command(about = "Scheduler-backed team orchestration and Kimi asset tooling")]
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
        Commands::Setup => run_setup().await,
        Commands::Update(args) => run_update(args).await,
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

async fn run_setup() -> Result<()> {
    info!("Running omk setup");

    crate::runtime::config::ensure_dirs().await?;

    let config_dir = crate::runtime::config::config_dir();
    let state_dir = crate::runtime::config::state_dir();
    let data_dir = crate::runtime::config::data_dir();

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
        crate::runtime::atomic::atomic_write(&config_path, default_config.as_bytes()).await?;
    }

    let skills_dir = data_dir.join("skills");
    tokio::fs::create_dir_all(&skills_dir).await?;

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
    println!("  2. Run 'omk team run 2:coder \"fix TypeScript errors\"' to try team mode");

    Ok(())
}

async fn run_update(args: UpdateArgs) -> Result<()> {
    use tokio::process::Command;

    let current = env!("CARGO_PKG_VERSION");
    println!("Current version: {current}");

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

    println!("Checking for latest release...");
    let latest = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        Command::new("curl")
            .args([
                "-fsSL",
                "-H",
                "Accept: application/vnd.github+json",
                "https://api.github.com/repos/ekhodzitsky/oh-my-kimi/releases/latest",
            ])
            .output(),
    )
    .await
    {
        Ok(Ok(out)) if out.status.success() => {
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

    let asset = format!("omk-{latest_version}-{target}.tar.gz");
    let base_url = format!("https://github.com/ekhodzitsky/oh-my-kimi/releases/download/{latest}");
    let url = format!("{base_url}/{asset}");
    let sha_url = format!("{base_url}/{asset}.sha256");

    println!("Downloading {url}...");

    let tmp_dir = tempfile::tempdir()?;
    let tar_path = tmp_dir.path().join(&asset);
    let sha_path = tmp_dir.path().join(format!("{asset}.sha256"));

    let download = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&tar_path)
            .arg(&url)
            .status(),
    )
    .await??;

    if !download.success() {
        anyhow::bail!("Download failed. Prebuilt binary may not be available for {target}.");
    }

    // SHA256 verification is mandatory. Without it, a MITM on the CDN or a
    // compromised release artifact would land arbitrary code on the host.
    println!("Fetching checksum...");
    let sha_download = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&sha_path)
            .arg(&sha_url)
            .status(),
    )
    .await??;

    if !sha_download.success() {
        anyhow::bail!(
            "Checksum file not found at {sha_url}. \
             Refusing to install an unverified binary. \
             Re-run with cargo: cargo install --git https://github.com/ekhodzitsky/oh-my-kimi.git"
        );
    }

    verify_sha256(&tar_path, &sha_path).await?;
    println!("✓ SHA256 verified");

    println!("Extracting...");
    let extract = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new("tar")
            .args(["--no-same-owner", "-xzf"])
            .arg(&tar_path)
            .arg("-C")
            .arg(tmp_dir.path())
            .status(),
    )
    .await??;

    if !extract.success() {
        anyhow::bail!("Failed to extract archive");
    }

    // Release tarballs are flat (`omk` at the root); the older nested layout
    // (`omk-<ver>-<target>/omk`) is kept as a fallback for transitional
    // versions still in the wild.
    let new_binary = tmp_dir.path().join("omk");
    if !new_binary.exists() {
        let legacy = tmp_dir
            .path()
            .join(format!("omk-{latest_version}-{target}"))
            .join("omk");
        if legacy.exists() {
            tokio::fs::copy(&legacy, &new_binary).await?;
        } else {
            anyhow::bail!("Could not find omk binary in downloaded archive");
        }
    }

    let current_exe = std::env::current_exe()?;
    println!("Replacing {}...", current_exe.display());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&new_binary).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&new_binary, perms).await?;
    }

    // Install atomically: write next to current_exe, fsync, rename. A partial
    // copy mid-write previously left users with a corrupt or zero-byte omk.
    install_binary_atomically(&new_binary, &current_exe).await?;

    println!("✓ Updated to {latest_version}");
    println!("  Binary: {}", current_exe.display());

    println!("Updating shell completions...");
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new(&current_exe)
            .args(["completions", "bash"])
            .output(),
    )
    .await;
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new(&current_exe)
            .args(["completions", "zsh"])
            .output(),
    )
    .await;
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        Command::new(&current_exe)
            .args(["completions", "fish"])
            .output(),
    )
    .await;

    println!("Run `omk doctor` to verify the installation.");
    Ok(())
}

/// Verify the SHA256 checksum of `archive_path` against the digest recorded
/// in `sha_path`.
///
/// The sha file is produced by `sha256sum` / `shasum -a 256` and follows the
/// `<hex-digest>  <basename>` format. We shell out to the same tool (either
/// `sha256sum` on Linux or `shasum -a 256` on macOS), running it with `cwd`
/// set to the archive's parent so the relative filename in the sha file
/// resolves correctly. Refusing to install on verification failure is
/// non-optional — the alternative is RCE-on-MITM.
async fn verify_sha256(archive_path: &std::path::Path, sha_path: &std::path::Path) -> Result<()> {
    use anyhow::Context;

    let parent = archive_path.parent().unwrap_or(std::path::Path::new("."));
    let sha_file_name = sha_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("checksum file path has no name"))?;
    let archive_name = archive_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("archive path has no name"))?
        .to_string_lossy()
        .into_owned();

    // Belt-and-braces: assert the .sha256 file actually references our
    // archive's basename, so a copy-paste mistake or a transitional rename
    // (sha file says `omk-0.3.30-...` but we downloaded `omk-0.3.31-...`)
    // fails loudly instead of silently passing if both happen to coexist in
    // the same directory.
    let sha_contents = tokio::fs::read_to_string(sha_path)
        .await
        .with_context(|| format!("failed to read checksum file {}", sha_path.display()))?;
    let mut referenced = false;
    for line in sha_contents.lines() {
        // Format: `<hex-digest>  <filename>` (sha256sum / shasum -a 256).
        if let Some(name) = line.split_whitespace().nth(1) {
            if name == archive_name {
                referenced = true;
                break;
            }
        }
    }
    if !referenced {
        anyhow::bail!(
            "Checksum file {} does not reference {}; refusing to verify against an unrelated digest",
            sha_path.display(),
            archive_name
        );
    }

    let mut tried_any = false;
    for cmd in [("sha256sum", vec!["-c"]), ("shasum", vec!["-a", "256", "-c"])] {
        if which::which(cmd.0).is_err() {
            continue;
        }
        tried_any = true;
        let status = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            Command::new(cmd.0)
                .args(&cmd.1)
                .arg(sha_file_name)
                .current_dir(parent)
                .status(),
        )
        .await
        .context("sha256 verification command timed out")?
        .context("failed to spawn sha256 verification command")?;

        if status.success() {
            return Ok(());
        }
        anyhow::bail!(
            "Checksum mismatch for {}; refusing to install an unverified binary",
            archive_path.display()
        );
    }

    if !tried_any {
        anyhow::bail!(
            "Neither sha256sum nor shasum is installed; cannot verify the download. \
             Install one and re-run, or use `cargo install`."
        );
    }
    unreachable!("loop body either returned Ok, bailed, or skipped");
}

/// Install `new_binary` as `current_exe` atomically.
///
/// Writes to a sibling `.omk.new` first, fsyncs, then renames into place.
/// Without this, a partial `tokio::fs::copy` (ENOSPC, signal, disk error)
/// previously left users with a corrupt or zero-byte omk. `rename` on the
/// same filesystem is atomic on Unix.
async fn install_binary_atomically(
    new_binary: &std::path::Path,
    current_exe: &std::path::Path,
) -> Result<()> {
    use anyhow::Context;

    let install_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("current_exe has no parent directory"))?;

    // Pre-flight check: a non-root user installing into /usr/local/bin will
    // hit EACCES on the rename below, with a downstream error that doesn't
    // point at the underlying permission problem. Catch it here with an
    // actionable message.
    let probe = install_dir.join(".omk.write-probe");
    if let Err(e) = tokio::fs::write(&probe, b"").await {
        anyhow::bail!(
            "No write access to {}: {}. Re-run with sudo, or install to a user-writable \
             location (e.g. cargo install path / ~/.local/bin / Homebrew prefix).",
            install_dir.display(),
            e
        );
    }
    let _ = tokio::fs::remove_file(&probe).await;

    let staging = install_dir.join(".omk.new");

    // Drop any prior staging file from a previous failed run.
    if staging.exists() {
        let _ = tokio::fs::remove_file(&staging).await;
    }

    tokio::fs::copy(new_binary, &staging)
        .await
        .with_context(|| format!("failed to stage new binary at {}", staging.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // 0o755 immediately after copy. Brief umask-default window between
        // copy and permission-set is harmless because (a) staging is in the
        // install dir, not a world-writable temp, and (b) the file is not
        // executable until the chmod completes, but it also isn't yet at
        // current_exe — nothing executes it.
        let mut perms = tokio::fs::metadata(&staging).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&staging, perms).await?;
    }

    // sync_data() flushes the staging file's contents to disk before the
    // rename swap, so a power-fail mid-install cannot leave behind a half-
    // written replacement.
    let staging_for_sync = staging.clone();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let f = std::fs::OpenOptions::new()
            .read(true)
            .open(&staging_for_sync)?;
        f.sync_data()
    })
    .await
    .context("sync task panicked")?
    .context("failed to fsync staged binary")?;

    tokio::fs::rename(&staging, current_exe).await.with_context(|| {
        format!(
            "failed to rename {} into {}",
            staging.display(),
            current_exe.display()
        )
    })?;

    Ok(())
}
