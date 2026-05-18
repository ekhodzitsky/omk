use anyhow::Result;

use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
use crate::runtime::goal::state::GOAL_CONTROLLER_ACTOR;
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};

pub(crate) async fn append_controller_task_events(
    state: &crate::runtime::goal::state::GoalState,
    task_graph: &GoalTaskGraph,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let mut events = Vec::new();

    for task in task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Done)
    {
        let task_id = TaskId(task.id.clone());
        events.push(
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "title": task.title,
                }))?,
        );
        events.push(builder.task_completed(
            task_id,
            worker_id.clone(),
            Some(&controller_task_summary(task)),
        )?);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}

pub(crate) fn controller_task_summary(task: &GoalTask) -> String {
    let artifacts = task
        .evidence
        .iter()
        .map(|evidence| evidence.path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} completed with artifact evidence: {}",
        task.id, artifacts
    )
}
