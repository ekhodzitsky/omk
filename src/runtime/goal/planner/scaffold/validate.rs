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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::goal::task_graph::{GoalTask, GoalTaskEvidence, GoalTaskStatus};
    use std::path::PathBuf;

    fn task_with_evidence(id: &str, evidence: Vec<GoalTaskEvidence>) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: id.to_string(),
            description: String::new(),
            status: GoalTaskStatus::Done,
            owner_role: None,
            completed_at: None,
            evidence,
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: vec![],
            read_set: vec![],
            write_set: vec![],
            risk: String::new(),
            acceptance: vec![],
        }
    }

    #[test]
    fn controller_task_summary_with_artifacts() {
        let task = task_with_evidence(
            "task-1",
            vec![
                GoalTaskEvidence {
                    kind: "file".to_string(),
                    path: PathBuf::from("src/main.rs"),
                    summary: "changed".to_string(),
                },
                GoalTaskEvidence {
                    kind: "file".to_string(),
                    path: PathBuf::from("src/lib.rs"),
                    summary: "changed".to_string(),
                },
            ],
        );
        let summary = controller_task_summary(&task);
        assert!(summary.contains("task-1"));
        assert!(summary.contains("src/main.rs"));
        assert!(summary.contains("src/lib.rs"));
    }

    #[test]
    fn controller_task_summary_without_artifacts() {
        let task = task_with_evidence("task-1", vec![]);
        let summary = controller_task_summary(&task);
        assert!(summary.contains("task-1"));
        assert!(summary.contains("completed with artifact evidence:"));
    }
}
