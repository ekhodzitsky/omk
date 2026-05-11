use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::kimi_native::role_packs::RolePack;
use crate::runtime::config::{EVENTS_FILE, TEAM_DIR, WORKERS_DIR};
use crate::runtime::events::{EventBuilder, EventKind, GateId, RunId};
use crate::runtime::gates::{run_gates_with_evidence, GateDef, VerificationConfig};
use crate::runtime::proof::ProofStatus;
use crate::runtime::sanitize::sanitize_name;
use crate::runtime::{events::EventWriter, state::TeamState, worker::WorkerSpec};

mod proof;
mod run_support;

use proof::{failure_artifact_path, finalize_team_run_proof};
use run_support::{
    detect_kimi_run_metadata, fallback_subtasks, setup_wire_workers, synthesize_results,
    WireWorkerSetup,
};

#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
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

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        TeamCommands::Run(args) => run_team(args).await,
        TeamCommands::List => list_teams().await,
        TeamCommands::Status(args) => status(args).await,
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

            if let Ok(state) = TeamState::load(&path).await {
                teams.push((name, state));
            }
        }
    }

    if teams.is_empty() {
        println!("No teams found.");
        return Ok(());
    }

    println!("Active teams:\n");
    println!("{:<20} {:<20} Task", "Name", "Phase");
    println!("{}", "─".repeat(78));

    for (name, state) in teams {
        println!(
            "{:<20} {:<20} {}",
            name,
            format!("{:?}", state.phase),
            state.task.chars().take(40).collect::<String>()
        );
    }

    println!("\nUse `omk team status <name>` for details.");
    Ok(())
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

    // Create Wire workers for scheduler-backed execution.
    let (worker_specs, wire_handles) = setup_wire_workers(WireWorkerSetup {
        team_name: &team_name,
        task: &task,
        count,
        role: &role,
        state_dir: &state_dir,
        dir: &args.dir,
        event_writer: &event_writer,
        run_id: &run_id,
    })
    .await?;

    // Build tasks from subtasks and initialize runner
    let tasks: Vec<crate::runtime::scheduler::task::Task> = subtasks
        .into_iter()
        .map(|s| {
            crate::runtime::scheduler::task::Task::new(&s.id, "subtask")
                .with_description(s.description)
                .with_read_set(s.read_set)
                .with_write_set(s.write_set)
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

    let run_succeeded = matches!(
        &run_result,
        Ok(summary) if summary.failed == 0 && summary.cancelled == 0
    );

    // Synthesize results if the scheduler returned worker output.
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
            if run_succeeded {
                println!("✓ Team run '{}' completed", team_name);
            } else {
                println!("✗ Team run '{}' finished with failures", team_name);
            }
            println!("  Completed: {}/{}", summary.completed, summary.total);
            if summary.failed > 0 {
                println!("  Failed:    {}", summary.failed);
            }
            if summary.cancelled > 0 {
                println!("  Cancelled: {}", summary.cancelled);
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

    if run_succeeded {
        run_verification_gates(&args.dir, &state_dir, &event_writer, &run_id, &args.gate).await;
    }

    let proof = finalize_team_run_proof(&state_dir, &event_writer, &run_id)
        .await
        .with_context(|| format!("failed to write proof artifact for team '{}'", team_name))?;
    println!("  Proof:   {} ({})", proof.status, proof.readiness());
    if proof.status != ProofStatus::Ready {
        println!("  Failure: {}", failure_artifact_path(&state_dir).display());
    }

    println!();
    println!("State:   {}", state_dir.display());

    Ok(())
}

async fn run_verification_gates(
    dir: &std::path::Path,
    state_dir: &std::path::Path,
    event_writer: &EventWriter,
    run_id: &RunId,
    selected: &[String],
) {
    let preset = vec![
        GateDef {
            name: "fmt".to_string(),
            command: "cargo".to_string(),
            args: vec!["fmt".to_string(), "--check".to_string()],
            required: true,
            timeout_secs: 120,
        },
        GateDef {
            name: "check".to_string(),
            command: "cargo".to_string(),
            args: vec!["check".to_string(), "--all-targets".to_string()],
            required: true,
            timeout_secs: 120,
        },
        GateDef {
            name: "clippy".to_string(),
            command: "cargo".to_string(),
            args: vec![
                "clippy".to_string(),
                "--all-targets".to_string(),
                "--all-features".to_string(),
                "--".to_string(),
                "-D".to_string(),
                "warnings".to_string(),
            ],
            required: true,
            timeout_secs: 120,
        },
        GateDef {
            name: "test".to_string(),
            command: "cargo".to_string(),
            args: vec!["test".to_string()],
            required: true,
            timeout_secs: 120,
        },
    ];

    let gates_to_run: Vec<_> = if selected.is_empty() {
        preset
    } else {
        preset
            .into_iter()
            .filter(|gate| selected.iter().any(|s| s == gate.name.as_str()))
            .collect()
    };

    if gates_to_run.is_empty() {
        return;
    }

    let artifacts_dir = state_dir.join("artifacts").join("gates");
    println!("Verification:");
    for gate in gates_to_run {
        let command_line = if gate.args.is_empty() {
            gate.command.clone()
        } else {
            format!("{} {}", gate.command, gate.args.join(" "))
        };
        let gate_id = GateId(gate.name.clone());
        let builder = EventBuilder::new(run_id.clone());

        if let Ok(event) = builder.command_started(
            gate_id.clone(),
            &gate.name,
            &command_line,
            gate.timeout_secs,
        ) {
            let _ = event_writer.append(&event).await;
        }

        let results = run_gates_with_evidence(
            &VerificationConfig {
                gates: vec![gate.clone()],
            },
            dir,
            Some(&artifacts_dir),
        )
        .await;

        if let Some(result) = results.first() {
            if let Ok(event) = builder.command_finished(
                gate_id.clone(),
                &result.name,
                &result.command_line,
                result.exit_code,
                result.timed_out,
                result.stdout_summary.as_deref(),
                result.stderr_summary.as_deref(),
                result.output_path.as_deref(),
            ) {
                let _ = event_writer.append(&event).await;
            }

            let gate_event = if result.passed {
                builder.gate_passed_with_evidence(
                    gate_id.clone(),
                    &result.name,
                    result.required,
                    Some(&result.command_line),
                    result.exit_code,
                    result.timed_out,
                    result.stdout_summary.as_deref(),
                    result.stderr_summary.as_deref(),
                    result.output_path.as_deref(),
                    Some(result.timeout_secs),
                )
            } else {
                builder.gate_failed_with_evidence(
                    gate_id.clone(),
                    &result.name,
                    result.required,
                    Some(&result.command_line),
                    result.exit_code,
                    result.timed_out,
                    result.stdout_summary.as_deref(),
                    result.stderr_summary.as_deref(),
                    result.output_path.as_deref(),
                    Some(result.timeout_secs),
                )
            };

            if let Ok(event) = gate_event {
                let _ = event_writer.append(&event).await;
            }

            if result.passed {
                println!("  {:<8} ✓", result.name);
            } else if result.timed_out {
                println!("  {:<8} ✗ (timeout {}s)", result.name, result.timeout_secs);
            } else if let Some(code) = result.exit_code {
                println!("  {:<8} ✗ (exit code {})", result.name, code);
            } else {
                println!("  {:<8} ✗ (command error)", result.name);
            }
        }
    }
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

    println!("Team:        {}", state.name);
    println!("Task:        {}", state.task);
    println!("Phase:       {:?}", state.phase);
    println!("Created:     {}", state.created_at);
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

    // Emit manual_interrupt event before killing
    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = EventWriter::new(&event_log);
    let run_id = RunId(team_name.clone());
    let interrupt_event =
        crate::runtime::events::Event::new(run_id.clone(), EventKind::ManualInterrupt)
            .with_actor("omk-cli");
    let _ = event_writer.append(&interrupt_event).await;

    if !args.force {
        println!("Marking team '{}' as interrupted...", team_name);
    }

    // Update state
    let mut state = TeamState::load(&state_dir).await?;
    state.phase = crate::runtime::state::TeamPhase::Shutdown;
    state.save().await?;

    if let Err(e) = finalize_team_run_proof(&state_dir, &event_writer, &run_id).await {
        warn!(error = %e, team = %team_name, "Failed to write shutdown proof artifact");
    }

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

    let watchdog = crate::runtime::watchdog::Watchdog::with_defaults();
    let report = watchdog
        .check_team(&run_id, &state_dir, &event_writer)
        .await?;

    println!("🩺 Health check — {}", report.run_id);
    println!("Checked at:  {}", report.checked_at);
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
            "  {} {:12} inbox={} outbox={}",
            status_icon, worker.worker_id, worker.inbox_count, worker.outbox_count
        );
        println!("     → {}", worker.message);
    }

    println!();

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
    println!("  Run `omk team status {}` to inspect it", state.name);
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
