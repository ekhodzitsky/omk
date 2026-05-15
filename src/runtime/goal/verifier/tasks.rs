use anyhow::Result;
use chrono::{DateTime, Utc};

use super::super::evidence::GoalReviewEvidence;
use super::super::state::{
    GoalState, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_CONTROLLER_ACTOR, GOAL_LOCAL_VERIFY_TASK_ID,
    GOAL_REVIEW_TASK_ID, GOAL_SECURITY_REVIEW_TASK_ID,
};
use super::super::task_graph::{
    goal_task_done, GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus,
};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};

pub(crate) fn apply_goal_review_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalReviewEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let review_ok = goal_task_done(task_graph, GOAL_LOCAL_VERIFY_TASK_ID)
        && goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID);
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_REVIEW_TASK_ID)?;

    task.status = if review_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = review_ok.then_some(completed_at);
    task.evidence = vec![GoalTaskEvidence {
        kind: "review".to_string(),
        path: evidence.review_path.clone(),
        summary: evidence.review_summary.clone(),
    }];
    Some(task.clone())
}

pub(crate) fn apply_goal_security_review_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalReviewEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let security_ok = goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID)
        && evidence.security_findings.is_empty();
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_SECURITY_REVIEW_TASK_ID)?;

    task.status = if security_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = security_ok.then_some(completed_at);
    task.evidence = vec![GoalTaskEvidence {
        kind: "security_review".to_string(),
        path: evidence.security_review_path.clone(),
        summary: evidence.security_summary.clone(),
    }];
    Some(task.clone())
}

pub(crate) async fn append_goal_review_task_events(
    state: &GoalState,
    tasks: &[GoalTask],
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let mut events = Vec::new();

    for task in tasks {
        let task_id = TaskId(task.id.clone());
        let summary = super::super::planner::controller_task_summary(task);
        events.push(
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "title": task.title,
                }))?,
        );
        let finished = if task.status == GoalTaskStatus::Done {
            builder.task_completed(task_id, worker_id.clone(), Some(&summary))?
        } else {
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskFailed)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "summary": summary,
                }))?
        };
        events.push(finished);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}
