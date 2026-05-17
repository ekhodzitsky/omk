use std::path::Path;

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use crate::runtime::events::{EventBuilder, EventWriter, RunId, WorkerId};
use crate::runtime::goal::dispatch::interrupt::watch_goal_control_interrupt;
use crate::runtime::goal::dispatch::runtime::{
    goal_agent_lease_seconds_override, goal_agent_worker_count, prepare_goal_agent_workers,
    stop_wire_worker,
};
use crate::runtime::goal::state::GOAL_AGENT_WORKER_ROLE;
use crate::runtime::scheduler::runner::{RunSummary, TeamRunner};
use crate::runtime::scheduler::task::Task;
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::WorkerSpec;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_wave_run(
    run_id: &str,
    project_dir: &Path,
    run_dir: &Path,
    state_dir: &Path,
    event_writer: EventWriter,
    builder: &EventBuilder,
    scheduler_tasks: Vec<Task>,
    max_agents: usize,
) -> Result<(RunSummary, Vec<WorkerSpec>)> {
    let worker_count = goal_agent_worker_count(max_agents, scheduler_tasks.len());
    let worker_specs = prepare_goal_agent_workers(run_dir, project_dir, worker_count).await?;
    for spec in &worker_specs {
        event_writer
            .append(&builder.worker_started(WorkerId(spec.name.clone()), GOAL_AGENT_WORKER_ROLE)?)
            .await?;
    }

    let mcp_bridge = crate::mcp::bridge::maybe_create_bridge().await;

    let cancel = CancellationToken::new();
    let mut handles = Vec::with_capacity(worker_specs.len());
    for spec in &worker_specs {
        let adapter = WireWorkerAdapter::new_with_cancel(
            spec.clone(),
            RunId(run_id.to_string()),
            event_writer.clone(),
            cancel.clone(),
        )
        .with_mcp_bridge(mcp_bridge.clone());
        handles.push(adapter.spawn());
    }

    let mut runner =
        TeamRunner::init_with_tasks(run_id, project_dir, run_dir, event_writer, scheduler_tasks)
            .await?;
    if let Some(lease_secs) = goal_agent_lease_seconds_override() {
        runner.set_lease_seconds(lease_secs);
    }

    let monitor_cancel = CancellationToken::new();
    let monitor_handle = tokio::spawn(watch_goal_control_interrupt(
        state_dir.to_path_buf(),
        cancel.clone(),
        monitor_cancel.clone(),
    ));

    let run_result = runner
        .run_with_cancel_reason(&worker_specs, &cancel, "cancelled by user")
        .await;
    monitor_cancel.cancel();
    let _ = monitor_handle.await;
    cancel.cancel();
    for handle in &mut handles {
        stop_wire_worker(handle).await;
    }

    Ok((run_result?, worker_specs))
}
