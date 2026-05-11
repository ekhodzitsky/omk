use anyhow::{Context, Result};

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::cli::team::args::RunArgs;
use crate::cli::team::proof::{failure_artifact_path, finalize_team_run_proof};
use crate::cli::team::run_support::{
    detect_kimi_run_metadata, fallback_subtasks, setup_wire_workers, synthesize_results,
    WireWorkerSetup,
};
use crate::runtime::config::{EVENTS_FILE, TEAM_DIR};
use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, GateId, RunId};
use crate::runtime::gates::{run_gates_with_evidence, GateDef, VerificationConfig};
use crate::runtime::proof::ProofStatus;
use crate::runtime::sanitize::sanitize_name;

pub(crate) async fn run_team(args: RunArgs, cancel: CancellationToken) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let (count, role) = super::args::parse_spec(&args.spec)?;
    if let Some(pack) = crate::kimi_native::role_packs::RolePack::find(&role) {
        info!("Using role pack: {} ({})", pack.name, pack.description);
    }
    let team_name = if let Some(ref name) = args.name {
        sanitize_name(name)?
    } else {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        format!("{}-{}", role, &suffix[..8])
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
        cancel_token: cancel.clone(),
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
    let (run_result, was_cancelled) = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            (Err(anyhow::anyhow!("run cancelled by user")), true)
        }
        res = runner.run(&worker_specs) => (res, false),
    };

    // Abort wire worker adapters
    for handle in wire_handles {
        handle.abort();
    }

    if was_cancelled {
        let interrupt_event =
            Event::new(run_id.clone(), EventKind::ManualInterrupt).with_actor("omk-cli");
        let _ = event_writer.append(&interrupt_event).await;
    }

    let run_succeeded = matches!(
        &run_result,
        Ok(summary) if summary.failed == 0 && summary.cancelled == 0
    );

    // Synthesize results if the scheduler returned worker output.
    let synthesis_summary = if run_result.is_ok() && !was_cancelled {
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

pub(crate) async fn run_verification_gates(
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
