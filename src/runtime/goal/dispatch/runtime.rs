use std::path::Path;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::runtime::config::{HEARTBEAT_FILE, INBOX_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::worker::WorkerSpec;

use super::{GOAL_AGENT_WORKER_ID, GOAL_AGENT_WORKER_ROLE};

pub(crate) fn goal_agent_wire_runtime_available() -> bool {
    std::env::var_os("MOCK_KIMI").is_some() || which::which("kimi").is_ok()
}

pub(crate) async fn stop_wire_worker(handle: &mut JoinHandle<()>) {
    if tokio::time::timeout(Duration::from_secs(2), &mut *handle)
        .await
        .is_err()
    {
        handle.abort();
        let _ = handle.await;
    }
}

pub(crate) fn goal_agent_worker_name(index: usize) -> String {
    if index == 0 {
        GOAL_AGENT_WORKER_ID.to_string()
    } else {
        format!("goal-agent-worker-{index}")
    }
}

pub(crate) fn goal_agent_worker_count(max_agents: usize, task_count: usize) -> usize {
    max_agents.max(1).min(task_count.max(1))
}

pub(crate) fn goal_agent_lease_seconds_override() -> Option<i64> {
    std::env::var("OMK_GOAL_AGENT_LEASE_SECS")
        .ok()
        .and_then(|raw| raw.trim().parse::<i64>().ok())
        .filter(|secs| *secs > 0)
}

pub(crate) async fn prepare_goal_agent_workers(
    run_dir: &Path,
    project_dir: &Path,
    worker_count: usize,
) -> anyhow::Result<Vec<WorkerSpec>> {
    let mut specs = Vec::with_capacity(worker_count);
    for index in 0..worker_count {
        let name = goal_agent_worker_name(index);
        let worker_dir = run_dir.join(WORKERS_DIR).join(&name);
        crate::runtime::config::ensure_private_dir(&worker_dir).await?;
        let spec = WorkerSpec {
            name,
            role: GOAL_AGENT_WORKER_ROLE.to_string(),
            inbox: worker_dir.join(INBOX_FILE),
            outbox: worker_dir.join(OUTBOX_FILE),
            heartbeat: worker_dir.join(HEARTBEAT_FILE),
            project_dir: Some(project_dir.to_path_buf()),
        };
        spec.save().await?;
        tokio::fs::write(&spec.inbox, b"").await?;
        tokio::fs::write(&spec.outbox, b"").await?;
        tokio::fs::write(worker_dir.join("wire-events.jsonl"), b"").await?;
        specs.push(spec);
    }
    Ok(specs)
}
