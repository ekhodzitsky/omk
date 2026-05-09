use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::kimi_native::role_packs::RolePack;
use crate::runtime::config::{EVENTS_FILE, TEAM_DIR, WORKERS_DIR};
use crate::runtime::events::{Event, EventBuilder, EventKind, RunId};
use crate::runtime::sanitize::sanitize_name;
use crate::runtime::{
    bridge::TeamBridge, events::EventWriter, state::TeamState, tmux,
    wire_worker::WireWorkerAdapter, worker::WorkerSpec,
};
use crate::skills::discovery::load_bundled_skill;

#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: TeamCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum TeamCommands {
    /// Spawn workers in tmux compatibility mode
    Spawn(SpawnArgs),
    /// Run a scheduler-backed team workflow (no tmux required)
    Run(RunArgs),
    /// List all active teams
    List,
    /// Check team status
    Status(StatusArgs),
    /// Attach to a team's tmux session
    Attach(AttachArgs),
    /// Broadcast a message to all team panes
    Broadcast(BroadcastArgs),
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
pub(crate) struct SpawnArgs {
    #[arg(value_name = "N:ROLE")]
    pub spec: Option<String>,

    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub task: Vec<String>,

    #[arg(short, long)]
    pub name: Option<String>,

    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    #[arg(long)]
    pub no_ralph: bool,

    #[arg(long)]
    pub yolo: bool,

    /// Skill to inject into lead prompt
    #[arg(short, long, default_value = "team")]
    pub skill: String,

    #[arg(long)]
    pub role_pack: Option<String>,
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
pub(crate) struct AttachArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct BroadcastArgs {
    #[arg(value_name = "NAME")]
    pub name: String,

    #[arg(trailing_var_arg = true, value_name = "MESSAGE")]
    pub message: Vec<String>,
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

    /// Attempt to recover dead or stalled workers
    #[arg(long)]
    pub recover: bool,
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

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        TeamCommands::Spawn(args) => spawn(args).await,
        TeamCommands::Run(args) => run_team(args).await,
        TeamCommands::List => list_teams().await,
        TeamCommands::Status(args) => status(args).await,
        TeamCommands::Attach(args) => attach(args).await,
        TeamCommands::Broadcast(args) => broadcast(args).await,
        TeamCommands::Rename(args) => rename_team(args).await,
        TeamCommands::Export(args) => export_team(args).await,
        TeamCommands::Import(args) => import_team(args).await,
        TeamCommands::Shutdown(args) => shutdown(args).await,
        TeamCommands::Health(args) => health(args).await,
        TeamCommands::Cleanup(args) => cleanup(args).await,
        TeamCommands::Roles => roles(),
    }
}

async fn list_teams() -> Result<()> {
    let teams_dir = crate::runtime::config::omk_state_dir().join(TEAM_DIR);

    if !teams_dir.exists() {
        println!("No teams found.");
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&teams_dir).await?;
    let mut teams = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let session_name = format!("omk-team-{}", name);
            let running = crate::runtime::tmux::session_exists(&session_name).unwrap_or(false);

            if let Ok(state) = TeamState::load(&path).await {
                teams.push((name, state, running));
            }
        }
    }

    if teams.is_empty() {
        println!("No teams found.");
        return Ok(());
    }

    println!("Active teams:\n");
    println!("{:<20} {:<8} {:<20} Task", "Name", "Running", "Phase");
    println!("{}", "─".repeat(90));

    for (name, state, running) in teams {
        let status = if running { "●" } else { "○" };
        println!(
            "{:<20} {:<8} {:<20} {}",
            name,
            status,
            format!("{:?}", state.phase),
            state.task.chars().take(40).collect::<String>()
        );
    }

    println!("\nUse `omk team status <name>` for details.");
    Ok(())
}

async fn spawn(mut args: SpawnArgs) -> Result<()> {
    if args.spec.is_none() && args.role_pack.is_none() {
        anyhow::bail!("Either spec (N:ROLE) or --role-pack is required");
    }

    // If --role-pack is provided and spec looks like a task word (not a valid spec),
    // prepend spec to task so the user can write: omk team spawn --role-pack architect test task
    if args.role_pack.is_some() && args.spec.is_some() {
        let spec = args.spec.as_ref().unwrap();
        if spec.contains(':') || resolve_role_alias(spec).is_some() {
            anyhow::bail!("Cannot use both --role-pack and N:ROLE spec");
        }
        let mut task = vec![args.spec.take().unwrap()];
        task.extend(args.task);
        args.task = task;
    }

    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let (count, role, pack_opt) = if let Some(ref role_pack_id) = args.role_pack {
        let pack = RolePack::find(role_pack_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown role pack: {}", role_pack_id))?;
        info!("Using role pack: {} ({})", pack.name, pack.description);
        (pack.suggested_worker_count, pack.id.clone(), Some(pack))
    } else if let Some(ref spec) = args.spec {
        let (count, role) = parse_spec(spec)?;
        let pack = RolePack::find(&role);
        if let Some(ref pack) = pack {
            info!("Using role pack: {} ({})", pack.name, pack.description);
        }
        (count, role, pack)
    } else {
        unreachable!()
    };

    let team_name = if let Some(ref name) = args.name {
        sanitize_name(name)?
    } else {
        format!(
            "{}-{}",
            role,
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        )
    };

    info!(team = %team_name, workers = count, role = role, task = %task, "Starting team");

    tmux::ensure_tmux()?;

    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);
    tokio::fs::create_dir_all(&state_dir).await?;

    // Initialize event logging
    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = EventWriter::new(&event_log);
    let run_id = RunId(team_name.clone());

    let run_started = EventBuilder::new(run_id.clone()).run_started("team", &args.dir, &task)?;
    event_writer.append(&run_started).await?;

    let spawn_result = spawn_inner(
        args,
        &team_name,
        &task,
        count,
        &role,
        pack_opt.as_ref(),
        &state_dir,
        &event_writer,
        &run_id,
    )
    .await;

    if let Err(ref e) = spawn_result {
        let run_failed =
            EventBuilder::new(run_id.clone()).run_failed(&format!("spawn failed: {}", e))?;
        let _ = event_writer.append(&run_failed).await;
    }

    spawn_result
}

async fn run_team(args: RunArgs) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let (count, role) = parse_spec(&args.spec)?;
    if let Some(pack) = RolePack::find(&role) {
        info!("Using role pack: {} ({})", pack.name, pack.description);
    }
    let team_name = if let Some(ref name) = args.name {
        sanitize_name(name)?
    } else {
        format!(
            "{}-{}",
            role,
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        )
    };

    info!(team = %team_name, workers = count, role = role, task = %task, "Running team");

    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);
    tokio::fs::create_dir_all(&state_dir).await?;

    // Initialize event logging
    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = EventWriter::new(&event_log);
    let run_id = RunId(team_name.clone());

    // Resolve kimi binary for lead decomposition / synthesis
    let kimi_bin = std::env::var("MOCK_KIMI")
        .ok()
        .or_else(|| {
            which::which("kimi")
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "kimi".to_string());

    let metadata = detect_kimi_run_metadata(&kimi_bin).await;
    let run_started = EventBuilder::new(run_id.clone()).run_started_with_kimi_metadata(
        "team",
        &args.dir,
        &task,
        Some(metadata.binary.clone()),
        metadata.cli_version.clone(),
        Some(metadata.wire_protocol_version.clone()),
    )?;
    event_writer.append(&run_started).await?;

    // Try lead decomposition
    let subtasks = match tokio::time::timeout(
        std::time::Duration::from_secs(60),
        crate::runtime::scheduler::decompose::LeadDecomposer::decompose(&task, count, &kimi_bin),
    )
    .await
    {
        Ok(Ok(tasks)) => tasks,
        Ok(Err(e)) => {
            warn!("Lead decomposition failed: {}", e);
            fallback_subtasks(&task, count)
        }
        Err(_) => {
            warn!("Lead decomposition timed out");
            fallback_subtasks(&task, count)
        }
    };

    // Create wire workers (no tmux, no bash bridge)
    let (worker_specs, wire_handles) = setup_wire_workers(
        &team_name,
        &task,
        count,
        &role,
        &state_dir,
        &args.dir,
        &event_writer,
        &run_id,
    )
    .await?;

    // Build tasks from subtasks and initialize runner
    let tasks: Vec<crate::runtime::scheduler::task::Task> = subtasks
        .into_iter()
        .map(|s| {
            crate::runtime::scheduler::task::Task::new(&s.id, "subtask")
                .with_description(&s.description)
        })
        .collect();

    let mut runner = crate::runtime::scheduler::runner::TeamRunner::init_with_tasks(
        &team_name,
        &args.dir,
        &state_dir,
        EventWriter::new(&event_log),
        tasks,
    )
    .await?;

    // Run the orchestration loop
    let run_result = runner.run(&worker_specs).await;

    // Synthesize results if run succeeded
    let synthesis_summary = if run_result.is_ok() {
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            synthesize_results(&worker_specs, &state_dir, &event_writer, &run_id, &kimi_bin),
        )
        .await
        {
            Ok(Ok(summary)) => Some(summary),
            Ok(Err(e)) => {
                warn!("Synthesis failed: {}", e);
                None
            }
            Err(_) => {
                warn!("Synthesis timed out");
                None
            }
        }
    } else {
        None
    };

    // Abort wire worker adapters
    for handle in wire_handles {
        handle.abort();
    }

    match &run_result {
        Ok(summary) => {
            let event = EventBuilder::new(run_id.clone()).run_completed();
            let _ = event_writer.append(&event).await;
            println!("✓ Team run '{}' completed", team_name);
            println!("  Completed: {}/{}", summary.completed, summary.total);
            if summary.failed > 0 {
                println!("  Failed:    {}", summary.failed);
            }
            if let Some(ref synth) = synthesis_summary {
                println!("  Synthesis: {}", synth);
            }
        }
        Err(e) => {
            let event =
                EventBuilder::new(run_id.clone()).run_failed(&format!("run failed: {}", e))?;
            let _ = event_writer.append(&event).await;
            println!("✗ Team run '{}' failed: {}", team_name, e);
        }
    }

    if run_result.is_ok() {
        run_verification_gates(&args.dir, &event_writer, &run_id, &args.gate).await;
    }

    println!();
    println!("State:   {}", state_dir.display());

    Ok(())
}

async fn run_verification_gates(
    dir: &std::path::Path,
    event_writer: &EventWriter,
    run_id: &RunId,
    selected: &[String],
) {
    let preset = vec![
        ("fmt", vec!["fmt", "--check"]),
        (
            "clippy",
            vec![
                "clippy",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("test", vec!["test"]),
    ];

    let gates_to_run: Vec<_> = if selected.is_empty() {
        preset
    } else {
        preset
            .into_iter()
            .filter(|(name, _)| selected.iter().any(|s| s == *name))
            .collect()
    };

    if gates_to_run.is_empty() {
        return;
    }

    println!("Verification:");
    for (name, args) in gates_to_run {
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.args(&args).current_dir(dir);

        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    if let Ok(event) = EventBuilder::new(run_id.clone()).gate_passed_by_name(name) {
                        let _ = event_writer.append(&event).await;
                    }
                    println!("  {:<8} ✓", name);
                } else {
                    if let Ok(event) = EventBuilder::new(run_id.clone()).gate_failed_by_name(name) {
                        let _ = event_writer.append(&event).await;
                    }
                    let code = output.status.code().unwrap_or(-1);
                    println!("  {:<8} ✗ (exit code {})", name, code);
                }
            }
            Err(e) => {
                if let Ok(event) = EventBuilder::new(run_id.clone()).gate_failed_by_name(name) {
                    let _ = event_writer.append(&event).await;
                }
                let reason = if e.kind() == std::io::ErrorKind::NotFound {
                    "command not found"
                } else {
                    "command error"
                };
                println!("  {:<8} ✗ ({})", name, reason);
            }
        }
    }
}

fn fallback_subtasks(
    task: &str,
    count: usize,
) -> Vec<crate::runtime::scheduler::decompose::Subtask> {
    (0..count)
        .map(|i| crate::runtime::scheduler::decompose::Subtask {
            id: format!("task-{}", i + 1),
            description: format!("{} — worker-{} focus", task, i),
        })
        .collect()
}

struct KimiRunMetadata {
    binary: String,
    cli_version: Option<String>,
    wire_protocol_version: String,
}

async fn detect_kimi_run_metadata(kimi_bin: &str) -> KimiRunMetadata {
    let cli_version = command_first_line(kimi_bin, &["--version"]).await;
    let wire_protocol_version = command_output(kimi_bin, &["info"])
        .await
        .and_then(|info| parse_wire_protocol_version(&info))
        .unwrap_or_else(|| crate::wire::protocol::KIMI_WIRE_PROTOCOL_VERSION.to_string());

    KimiRunMetadata {
        binary: kimi_bin.to_string(),
        cli_version,
        wire_protocol_version,
    }
}

async fn command_first_line(binary: &str, args: &[&str]) -> Option<String> {
    command_output(binary, args)
        .await
        .map(|text| text.lines().next().unwrap_or(&text).trim().to_string())
}

async fn command_output(binary: &str, args: &[&str]) -> Option<String> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new(binary).args(args).output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    (!text.is_empty()).then(|| text.to_string())
}

fn parse_wire_protocol_version(info_output: &str) -> Option<String> {
    for line in info_output.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("wire protocol") {
            return line
                .split([':', '='])
                .nth(1)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        }
    }
    None
}

async fn synthesize_results(
    worker_specs: &[WorkerSpec],
    state_dir: &std::path::Path,
    event_writer: &EventWriter,
    run_id: &RunId,
    kimi_bin: &str,
) -> Result<String> {
    let mut worker_results = Vec::new();
    for spec in worker_specs {
        if !spec.outbox.exists() {
            continue;
        }
        let content = tokio::fs::read_to_string(&spec.outbox).await?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(result) = serde_json::from_str::<crate::runtime::worker::WorkerResult>(line) {
                worker_results.push(format!(
                    "{} ({}): {}",
                    spec.name, result.task_id, result.summary
                ));
            }
        }
    }

    if worker_results.is_empty() {
        return Ok("No worker results available.".to_string());
    }

    let results_text = worker_results.join("\n");
    let prompt = format!(
        "You are a synthesis agent. The following subtasks were completed by a team of workers:\n{}\n\nSynthesize a concise final summary (2-3 sentences) of what was accomplished.",
        results_text
    );

    let synthesis =
        crate::runtime::scheduler::decompose::SynthesisAgent::synthesize(&prompt, kimi_bin).await?;

    let synthesis_path = state_dir.join("synthesis.txt");
    tokio::fs::write(&synthesis_path, &synthesis).await?;

    let event = Event::new(run_id.clone(), EventKind::TaskCompleted)
        .with_actor("synthesis-agent")
        .with_payload(serde_json::json!({
            "task_id": "synthesis",
            "summary": &synthesis,
        }))?;
    event_writer.append(&event).await?;

    Ok(synthesis)
}

#[allow(clippy::too_many_arguments)]
async fn setup_workers(
    team_name: &str,
    task: &str,
    count: usize,
    role: &str,
    state_dir: &std::path::Path,
    dir: &std::path::Path,
    event_writer: &EventWriter,
    run_id: &RunId,
) -> Result<Vec<WorkerSpec>> {
    let state = TeamState::new(team_name, task, state_dir, count, role);
    state.save().await?;

    let session_name = format!("omk-team-{team_name}");
    let window_name = "lead";

    if !tmux::session_exists(&session_name)? {
        tmux::create_session(&session_name, window_name, dir)?;
    }

    let mut worker_specs = Vec::new();

    for i in 0..count {
        let worker_name = format!("worker-{i}");
        let worker_dir = state_dir.join(WORKERS_DIR).join(&worker_name);
        tokio::fs::create_dir_all(&worker_dir).await?;

        tmux::split_window(&session_name, window_name, dir)?;
        tmux::rename_pane(&session_name, window_name, i + 1, &worker_name)?;

        let worker_spec = WorkerSpec {
            name: worker_name.clone(),
            role: role.to_string(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
            project_dir: Some(dir.to_path_buf()),
        };
        worker_spec.save().await?;
        worker_specs.push(worker_spec.clone());

        let bridge = TeamBridge::new(&worker_spec, &session_name);
        bridge.spawn_worker(i + 1).await?;

        let worker_started = EventBuilder::new(run_id.clone())
            .worker_started(crate::runtime::events::WorkerId(worker_name.clone()), role)?;
        event_writer.append(&worker_started).await?;
    }

    tmux::select_layout(&session_name, window_name, "tiled")?;

    Ok(worker_specs)
}

#[allow(clippy::too_many_arguments)]
async fn setup_wire_workers(
    team_name: &str,
    task: &str,
    count: usize,
    role: &str,
    state_dir: &std::path::Path,
    dir: &std::path::Path,
    event_writer: &EventWriter,
    run_id: &RunId,
) -> Result<(Vec<WorkerSpec>, Vec<tokio::task::JoinHandle<()>>)> {
    let state = TeamState::new(team_name, task, state_dir, count, role);
    state.save().await?;

    let mut worker_specs = Vec::new();
    let mut handles = Vec::new();

    for i in 0..count {
        let worker_name = format!("worker-{i}");
        let worker_dir = state_dir.join(WORKERS_DIR).join(&worker_name);
        tokio::fs::create_dir_all(&worker_dir).await?;

        let worker_spec = WorkerSpec {
            name: worker_name.clone(),
            role: role.to_string(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
            project_dir: Some(dir.to_path_buf()),
        };
        worker_spec.save().await?;
        worker_specs.push(worker_spec.clone());

        // Spawn wire worker adapter as a tokio task
        let adapter = WireWorkerAdapter::new(worker_spec, run_id.clone(), event_writer.clone());
        let handle = adapter.spawn();
        handles.push(handle);

        let worker_started = EventBuilder::new(run_id.clone())
            .worker_started(crate::runtime::events::WorkerId(worker_name.clone()), role)?;
        event_writer.append(&worker_started).await?;
    }

    Ok((worker_specs, handles))
}

#[allow(clippy::too_many_arguments)]
async fn spawn_inner(
    args: SpawnArgs,
    team_name: &str,
    task: &str,
    count: usize,
    role: &str,
    role_pack: Option<&RolePack>,
    state_dir: &std::path::Path,
    event_writer: &EventWriter,
    run_id: &RunId,
) -> Result<()> {
    let _worker_specs = setup_workers(
        team_name,
        task,
        count,
        role,
        state_dir,
        &args.dir,
        event_writer,
        run_id,
    )
    .await?;

    let session_name = format!("omk-team-{team_name}");
    let window_name = "lead";

    let mut skill_md = load_bundled_skill(&args.skill).await.unwrap_or_default();
    if let Some(pack) = role_pack {
        for skill_name in &pack.default_skills {
            if let Ok(content) = load_bundled_skill(skill_name).await {
                if !skill_md.is_empty() {
                    skill_md.push('\n');
                    skill_md.push('\n');
                }
                skill_md.push_str(&content);
            }
        }
    }

    // Load AGENTS.md context if available
    let agents_context =
        if let Ok(Some(manifest)) = crate::agents::runtime::load_project_agents(&args.dir).await {
            Some(crate::agents::runtime::inject_agents_context(
                &manifest, task, role,
            ))
        } else {
            None
        };

    let lead_prompt = build_lead_prompt(
        task,
        count,
        role,
        state_dir,
        args.yolo,
        &skill_md,
        agents_context.as_deref(),
        role_pack.map(|p| &*p.system_prompt),
    );
    crate::runtime::shell::validate_safe(&lead_prompt)
        .map_err(|e| anyhow::anyhow!("Invalid prompt: {}", e))?;
    tmux::send_keys(
        &session_name,
        window_name,
        &format!(
            "kimi -p {}",
            crate::runtime::shell::shell_escape(&lead_prompt)
        ),
    )?;

    // Record metrics
    let _ = crate::runtime::metrics::record(
        &crate::runtime::config::state_dir().join("metrics.json"),
        |m| m.total_spawns += 1,
    )
    .await;

    // Send notification
    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();
    if let Some(webhooks) = config.webhooks {
        crate::notifications::send_notification(
            &webhooks,
            &crate::notifications::NotificationEvent::TeamSpawned {
                name: team_name.to_string(),
                task: task.to_string(),
                workers: count,
                role: role.to_string(),
            },
        )
        .await;
    }

    println!(
        "✓ Team '{}' started with {} {} worker(s)",
        team_name, count, role
    );
    println!("  Session: {}", session_name);
    println!("  State:   {}", state_dir.display());
    println!();
    println!("Commands:");
    println!("  omk team status {team_name}");
    println!("  omk team shutdown {team_name}");
    println!();
    println!("Attach with: tmux attach -t {}", session_name);

    Ok(())
}

async fn status(args: StatusArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let state = TeamState::load(&state_dir).await?;
    let session_name = format!("omk-team-{}", team_name);
    let tmux_alive = tmux::session_exists(&session_name).unwrap_or(false);

    println!("Team:        {}", state.name);
    println!("Task:        {}", state.task);
    println!("Phase:       {:?}", state.phase);
    println!("Created:     {}", state.created_at);
    println!(
        "Tmux:        {}",
        if tmux_alive { "running" } else { "not found" }
    );
    println!();
    println!("Workers:");

    let workers_dir = state_dir.join(WORKERS_DIR);
    if workers_dir.exists() {
        let mut entries = tokio::fs::read_dir(&workers_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let worker_dir = entry.path();
            let spec_path = worker_dir.join("worker-spec.json");
            if !spec_path.exists() {
                continue;
            }

            let spec: WorkerSpec = {
                let json = tokio::fs::read_to_string(&spec_path).await?;
                serde_json::from_str(&json)?
            };

            let hb_status = if spec.heartbeat.exists() {
                match tokio::fs::read_to_string(&spec.heartbeat).await {
                    Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                        Ok(v) => v
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        Err(_) => "invalid".to_string(),
                    },
                    Err(_) => "unreadable".to_string(),
                }
            } else {
                "missing".to_string()
            };

            let inbox_count = count_jsonl_lines(&spec.inbox).await;
            let outbox_count = count_jsonl_lines(&spec.outbox).await;

            println!(
                "  {:12} role={:10} hb={:8} inbox={:2} outbox={:2}",
                spec.name, spec.role, hb_status, inbox_count, outbox_count
            );
        }
    }

    println!();
    println!("Tasks:       {} total", state.tasks.len());
    for task in &state.tasks {
        println!("  [{:?}] {}", task.status, task.description);
    }

    Ok(())
}

async fn shutdown(args: ShutdownArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let session_name = format!("omk-team-{}", team_name);

    // Emit manual_interrupt event before killing
    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = EventWriter::new(&event_log);
    let run_id = RunId(team_name.clone());
    let interrupt_event =
        crate::runtime::events::Event::new(run_id.clone(), EventKind::ManualInterrupt)
            .with_actor("omk-cli");
    let _ = event_writer.append(&interrupt_event).await;

    if tmux::session_exists(&session_name)? {
        if !args.force {
            println!("Sending interrupt to team '{}'...", team_name);
            // Send Ctrl-C to all panes
            let _ = tmux::send_keys(&session_name, "lead", "C-c");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        tmux::kill_session(&session_name)?;
        println!("✓ Tmux session '{}' killed", session_name);
    } else {
        println!(
            "⚠ Tmux session '{}' not found (already dead?)",
            session_name
        );
    }

    // Update state
    let mut state = TeamState::load(&state_dir).await?;
    state.phase = crate::runtime::state::TeamPhase::Shutdown;
    state.save().await?;

    // Estimate and record cost
    let duration = chrono::Utc::now().signed_duration_since(state.created_at);
    let cost_estimate = crate::cost::estimator::estimate_team_cost(
        duration.num_seconds().max(0) as u64,
        state.worker_count,
        &state.worker_role,
    );

    let _ = crate::runtime::session::record_session_end(
        "team",
        &team_name,
        state.created_at,
        cost_estimate.clone(),
        crate::notifications::NotificationEvent::TeamShutdown {
            name: team_name.clone(),
            duration_secs: 0,
            status: if args.force { "forced" } else { "graceful" }.to_string(),
        },
    )
    .await;

    // Record metrics
    let _ = crate::runtime::metrics::record(
        &crate::runtime::config::state_dir().join("metrics.json"),
        |m| m.total_shutdowns += 1,
    )
    .await;

    println!("✓ Team '{}' shut down", team_name);
    println!("  State:   {}", state_dir.display());
    println!("  Cost:    {}", cost_estimate.formatted());

    Ok(())
}

async fn cleanup(args: CleanupArgs) -> Result<()> {
    let teams_dir = crate::runtime::config::omk_state_dir().join(TEAM_DIR);
    let older_than = if args.all {
        None
    } else {
        Some(args.older_than)
    };
    let (removed, freed) =
        crate::cli::cleanup::cleanup_team_states(&teams_dir, older_than, args.dry_run).await?;
    println!();
    if args.dry_run {
        println!(
            "Would remove {removed} team state directories ({:.1} MB)",
            freed as f64 / 1_048_576.0
        );
    } else {
        println!(
            "Removed {removed} team state directories ({:.1} MB freed)",
            freed as f64 / 1_048_576.0
        );
    }
    Ok(())
}

async fn health(args: HealthArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = crate::runtime::events::EventWriter::new(&event_log);
    let run_id = crate::runtime::events::RunId(team_name.clone());

    let mut config = crate::runtime::watchdog::WatchdogConfig {
        require_tmux: true,
        ..Default::default()
    };
    if args.recover {
        config.attempt_recovery = true;
    }
    let watchdog = crate::runtime::watchdog::Watchdog::new(config);
    let report = watchdog
        .check_team(&run_id, &state_dir, &event_writer)
        .await?;

    println!("🩺 Health check — {}", report.run_id);
    println!("Checked at:  {}", report.checked_at);
    println!(
        "Tmux:        {}",
        if report.tmux_session_alive {
            "alive"
        } else {
            "missing"
        }
    );
    println!("Workers:     {}", report.workers.len());
    println!();

    for worker in &report.workers {
        let status_icon = match worker.status {
            crate::runtime::watchdog::HealthStatus::Healthy => "✅",
            crate::runtime::watchdog::HealthStatus::Stalled => "⚠️",
            crate::runtime::watchdog::HealthStatus::Dead => "❌",
            crate::runtime::watchdog::HealthStatus::Unknown => "❓",
        };
        println!(
            "  {} {:12} hb={} inbox={} outbox={}",
            status_icon,
            worker.worker_id,
            worker.tmux_pane_alive,
            worker.inbox_count,
            worker.outbox_count
        );
        println!("     → {}", worker.message);
    }

    println!();

    let mut recovery_performed = false;
    if args.recover && report.issues_found > 0 {
        println!("🔧 Attempting recovery...");
        for worker in &report.workers {
            if worker.status == crate::runtime::watchdog::HealthStatus::Dead
                || worker.status == crate::runtime::watchdog::HealthStatus::Stalled
            {
                let result = watchdog
                    .attempt_recovery(worker, &state_dir, &event_writer, &run_id)
                    .await?;
                recovery_performed = true;
                let icon = if result.success { "✅" } else { "❌" };
                println!(
                    "  {} {} — {}: {}",
                    icon, result.worker_id, result.action, result.message
                );
            }
        }
    }

    if recovery_performed {
        println!();
        println!("🩺 Re-running health check after recovery...");
        println!();
        let report = watchdog
            .check_team(&run_id, &state_dir, &event_writer)
            .await?;

        for worker in &report.workers {
            let status_icon = match worker.status {
                crate::runtime::watchdog::HealthStatus::Healthy => "✅",
                crate::runtime::watchdog::HealthStatus::Stalled => "⚠️",
                crate::runtime::watchdog::HealthStatus::Dead => "❌",
                crate::runtime::watchdog::HealthStatus::Unknown => "❓",
            };
            println!(
                "  {} {:12} hb={} inbox={} outbox={}",
                status_icon,
                worker.worker_id,
                worker.tmux_pane_alive,
                worker.inbox_count,
                worker.outbox_count
            );
            println!("     → {}", worker.message);
        }
        println!();
    }

    if report.issues_found == 0 {
        println!("🎉 All workers healthy.");
    } else {
        println!(
            "⚠️  {} issue(s) found. Check events.jsonl for details.",
            report.issues_found
        );
    }

    Ok(())
}

async fn count_jsonl_lines(path: &PathBuf) -> usize {
    if !path.exists() {
        return 0;
    }
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content.lines().filter(|l| !l.trim().is_empty()).count(),
        Err(_) => 0,
    }
}

async fn attach(args: AttachArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let session_name = format!("omk-team-{}", team_name);

    // Check if session exists
    let output = tokio::process::Command::new("tmux")
        .args(["has-session", "-t", &session_name])
        .output()
        .await
        .context("Failed to check tmux session")?;

    if !output.status.success() {
        anyhow::bail!(
            "Team '{}' is not running (tmux session '{}' not found)",
            team_name,
            session_name
        );
    }

    println!("Attaching to team '{}'...", team_name);
    println!("(Press Ctrl+B then D to detach)");

    // Replace current process with tmux attach on Unix
    #[cfg(unix)]
    {
        let err = std::process::Command::new("tmux")
            .args(["attach-session", "-t", &session_name])
            .exec();
        anyhow::bail!("Failed to attach to tmux session: {}", err)
    }

    #[cfg(not(unix))]
    {
        let status = tokio::process::Command::new("tmux")
            .args(["attach-session", "-t", &session_name])
            .status()
            .await
            .context("Failed to attach to tmux session")?;
        if !status.success() {
            anyhow::bail!("tmux attach exited with code {:?}", status.code());
        }
        Ok(())
    }
}

async fn broadcast(args: BroadcastArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let session_name = format!("omk-team-{}", team_name);
    let message = args.message.join(" ");

    if message.is_empty() {
        anyhow::bail!("Message is required");
    }

    // Check if session exists
    let output = tokio::process::Command::new("tmux")
        .args(["has-session", "-t", &session_name])
        .output()
        .await
        .context("Failed to check tmux session")?;

    if !output.status.success() {
        anyhow::bail!(
            "Team '{}' is not running (tmux session '{}' not found)",
            team_name,
            session_name
        );
    }

    // Get list of panes
    let pane_list = tokio::process::Command::new("tmux")
        .args(["list-panes", "-t", &session_name, "-F", "#{pane_index}"])
        .output()
        .await
        .context("Failed to list tmux panes")?;

    if !pane_list.status.success() {
        anyhow::bail!("Failed to list tmux panes");
    }

    let pane_output = String::from_utf8_lossy(&pane_list.stdout);
    let panes: Vec<&str> = pane_output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let escaped = crate::runtime::shell::shell_escape(&message);
    for pane in &panes {
        let target = format!("{}.{}", session_name, pane);
        let result = tokio::process::Command::new("tmux")
            .args(["send-keys", "-t", &target, &escaped, "C-m"])
            .output()
            .await;

        if let Err(e) = result {
            println!("  ⚠ Failed to send to pane {}: {}", pane, e);
        }
    }

    println!(
        "✓ Broadcasted to {} pane(s) in team '{}'",
        panes.len(),
        team_name
    );
    Ok(())
}

async fn rename_team(args: RenameArgs) -> Result<()> {
    let old_name = sanitize_name(&args.old_name)?;
    let new_name = sanitize_name(&args.new_name)?;
    let state_dir = crate::runtime::config::omk_state_dir().join(TEAM_DIR);
    let old_path = state_dir.join(&old_name);
    let new_path = state_dir.join(&new_name);

    if !old_path.exists() {
        anyhow::bail!("Team '{}' not found", old_name);
    }

    if new_path.exists() {
        anyhow::bail!("Team '{}' already exists", new_name);
    }

    // Check if tmux session is running
    let old_session = format!("omk-team-{}", old_name);
    let running = crate::runtime::tmux::session_exists(&old_session).unwrap_or(false);

    if running {
        // Rename tmux session
        let result = tokio::process::Command::new("tmux")
            .args([
                "rename-session",
                "-t",
                &old_session,
                &format!("omk-team-{}", new_name),
            ])
            .output()
            .await
            .context("Failed to rename tmux session")?;

        if !result.status.success() {
            anyhow::bail!(
                "Failed to rename tmux session: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }

    // Rename state directory
    tokio::fs::rename(&old_path, &new_path).await?;

    // Update state file
    let state_file = new_path.join("team-state.json");
    if let Ok(content) = tokio::fs::read_to_string(&state_file).await {
        if let Ok(mut state) = serde_json::from_str::<crate::runtime::state::TeamState>(&content) {
            state.name = new_name.clone();
            state.save().await?;
        }
    }

    println!("✓ Renamed team '{}' → '{}'", old_name, new_name);
    if !running {
        println!("  (tmux session was not running, only state was renamed)");
    }
    Ok(())
}

async fn export_team(args: ExportArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);
    let state_file = state_dir.join("team-state.json");

    if !state_file.exists() {
        anyhow::bail!("Team '{}' not found", team_name);
    }

    let content = tokio::fs::read_to_string(&state_file).await?;
    let state: crate::runtime::state::TeamState = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse team state for '{}'", team_name))?;

    let export = serde_json::json!({
        "version": "1.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "team": state,
    });

    let json = serde_json::to_string_pretty(&export)?;
    crate::runtime::atomic::atomic_write(std::path::Path::new(&args.output), json.as_bytes())
        .await?;

    println!("✓ Exported team '{}' to {}", team_name, args.output);
    Ok(())
}

async fn import_team(args: ImportArgs) -> Result<()> {
    let content = tokio::fs::read_to_string(&args.file)
        .await
        .with_context(|| format!("Failed to read file '{}'", args.file))?;
    let export: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON from '{}'", args.file))?;

    let team_value = export
        .get("team")
        .ok_or_else(|| anyhow::anyhow!("Invalid export file: missing 'team' field"))?;
    let state: crate::runtime::state::TeamState = serde_json::from_value(team_value.clone())
        .with_context(|| "Failed to deserialize team state")?;

    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&state.name);
    tokio::fs::create_dir_all(&state_dir).await?;

    state.save().await?;

    println!("✓ Imported team '{}' from {}", state.name, args.file);
    println!("  State dir: {}", state_dir.display());
    println!("  Run `omk team attach {}` to connect", state.name);
    Ok(())
}

fn parse_spec(spec: &str) -> Result<(usize, String)> {
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

#[allow(clippy::too_many_arguments)]
fn build_lead_prompt(
    task: &str,
    count: usize,
    role: &str,
    state_dir: &std::path::Path,
    yolo: bool,
    skill_md: &str,
    agents_context: Option<&str>,
    system_prompt: Option<&str>,
) -> String {
    let mut prompt = format!(
        r#"You are the Lead Orchestrator of a team of {count} {role} agent(s).

Your task: {task}

## Orchestration Skill
{skill_md}

## State Directory
{state_dir}

## Worker Inbox/Outbox Paths
{inbox_dir}

## Available Tools
- ReadFile / WriteFile for inbox/outbox
- Shell for tmux commands if needed
- TaskList / TaskOutput for background work
"#,
        count = count,
        role = role,
        task = task,
        skill_md = skill_md,
        state_dir = state_dir.display(),
        inbox_dir = state_dir.join(WORKERS_DIR).display(),
    );

    if let Some(ctx) = agents_context {
        prompt.push_str("\n## Project Context (from AGENTS.md)\n\n");
        prompt.push_str(ctx);
    }

    if let Some(sp) = system_prompt {
        prompt.push_str("\n## Role System Prompt\n\n");
        prompt.push_str(sp);
    }

    if yolo {
        prompt.push_str("\n\nYOLO mode is enabled. Auto-approve safe operations.\n");
    }

    prompt
}

fn roles() -> Result<()> {
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
