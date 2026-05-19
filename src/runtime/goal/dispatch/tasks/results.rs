use anyhow::Result;

use crate::runtime::config::EVENTS_FILE;
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
use crate::runtime::goal::evidence::GoalAgentRunEvidence;
use crate::runtime::goal::planner::controller_task_summary;
use crate::runtime::goal::state::{GoalState, GOAL_AGENT_WORKER_ID};
use crate::runtime::goal::task_graph::GoalTaskStatus;
use crate::runtime::worker::WorkerSpec;

pub(super) async fn read_goal_agent_worker_results(
    specs: &[WorkerSpec],
    task_ids: &[String],
) -> Result<Vec<crate::runtime::worker::WorkerResult>> {
    let mut filtered = Vec::new();
    for spec in specs {
        let results: Vec<crate::runtime::worker::WorkerResult> = spec.read_results().await?;
        filtered.extend(
            results
                .into_iter()
                .filter(|result| task_ids.iter().any(|task_id| task_id == &result.task_id)),
        );
    }
    Ok(filtered)
}

pub(super) fn summarize_goal_agent_worker_results(
    results: &[crate::runtime::worker::WorkerResult],
) -> Option<String> {
    let summaries: Vec<String> = results
        .iter()
        .map(|result| format!("{}: {}", result.task_id, result.summary))
        .collect();
    (!summaries.is_empty()).then(|| summaries.join(" | "))
}

pub(crate) async fn append_agent_execution_task_events(
    state: &GoalState,
    task: &crate::runtime::goal::task_graph::GoalTask,
    evidence: &GoalAgentRunEvidence,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(EVENTS_FILE));
    let run_id = RunId(state.goal_id.clone());
    let task_id = TaskId(task.id.clone());
    let worker_id = WorkerId(GOAL_AGENT_WORKER_ID.to_string());
    let summary = format!(
        "{} via {} (run: {}, scheduler: {})",
        controller_task_summary(task),
        GOAL_AGENT_WORKER_ID,
        evidence.run_path.display(),
        evidence.summary.run_id
    );
    let event = if task.status == GoalTaskStatus::Done {
        EventBuilder::new(run_id).task_completed(task_id, worker_id, Some(&summary))?
    } else {
        Event::new(run_id, EventKind::TaskFailed)
            .with_actor(GOAL_AGENT_WORKER_ID)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": GOAL_AGENT_WORKER_ID,
                "summary": summary,
            }))?
    };
    writer.append(&event).await
}
