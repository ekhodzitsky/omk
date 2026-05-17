use anyhow::Result;
use std::path::Path;

use crate::runtime::config::WORKERS_DIR;
use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId, WorkerId};
use crate::runtime::scheduler::decompose::{Subtask, SynthesisAgent};
use crate::runtime::state::TeamState;
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::{WorkerResult, WorkerSpec};

pub(super) struct KimiRunMetadata {
    pub(super) binary: String,
    pub(super) cli_version: Option<String>,
    pub(super) wire_protocol_version: String,
}

pub(super) async fn detect_kimi_run_metadata(kimi_bin: &str) -> KimiRunMetadata {
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

pub(super) fn fallback_subtasks(task: &str, count: usize) -> Vec<Subtask> {
    (0..count)
        .map(|i| Subtask {
            id: format!("task-{}", i + 1),
            description: format!("{} — worker-{} focus", task, i),
            read_set: Vec::new(),
            write_set: Vec::new(),
        })
        .collect()
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

pub(super) async fn synthesize_results(
    worker_specs: &[WorkerSpec],
    state_dir: &Path,
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
            if let Ok(result) = serde_json::from_str::<WorkerResult>(line) {
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

    let synthesis = SynthesisAgent::synthesize(&prompt, kimi_bin).await?;

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

pub(super) struct WireWorkerSetup<'a> {
    pub(super) team_name: &'a str,
    pub(super) task: &'a str,
    pub(super) count: usize,
    pub(super) role: &'a str,
    pub(super) state_dir: &'a Path,
    pub(super) dir: &'a Path,
    pub(super) event_writer: &'a EventWriter,
    pub(super) run_id: &'a RunId,
    pub(super) cancel_token: tokio_util::sync::CancellationToken,
}

pub(super) async fn setup_wire_workers(
    config: WireWorkerSetup<'_>,
) -> Result<(Vec<WorkerSpec>, Vec<tokio::task::JoinHandle<()>>)> {
    let state = TeamState::new(
        config.team_name,
        config.task,
        config.state_dir,
        config.count,
        config.role,
    );
    state.save().await?;

    let mcp_bridge = crate::mcp::bridge::maybe_create_bridge().await;

    let mut worker_specs = Vec::new();
    let mut handles = Vec::new();

    for i in 0..config.count {
        let worker_name = format!("worker-{i}");
        let worker_dir = config.state_dir.join(WORKERS_DIR).join(&worker_name);
        tokio::fs::create_dir_all(&worker_dir).await?;

        let worker_spec = WorkerSpec {
            name: worker_name.clone(),
            role: config.role.to_string(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
            project_dir: Some(config.dir.to_path_buf()),
            external_tools: None,
        };
        worker_spec.save().await?;
        worker_specs.push(worker_spec.clone());

        let adapter = WireWorkerAdapter::new_with_cancel(
            worker_spec,
            config.run_id.clone(),
            config.event_writer.clone(),
            config.cancel_token.clone(),
        )
        .with_mcp_bridge(mcp_bridge.clone());
        let handle = adapter.spawn();
        handles.push(handle);

        let worker_started = EventBuilder::new(config.run_id.clone())
            .worker_started(WorkerId(worker_name.clone()), config.role)?;
        config.event_writer.append(&worker_started).await?;
    }

    Ok((worker_specs, handles))
}
