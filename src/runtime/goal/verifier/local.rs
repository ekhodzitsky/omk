use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::runtime::goal::state::{GoalState, GOAL_CONTROLLER_ACTOR, GOAL_LOCAL_VERIFY_TASK_ID};
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, GateId, RunId, TaskId, WorkerId,
};
use crate::runtime::gates::{gates_passed, GateResult};

pub(crate) fn apply_local_verification_task_result(
    task_graph: &mut GoalTaskGraph,
    gates: &[GateResult],
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let gates_ok = !gates.is_empty() && gates_passed(gates);
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID)?;

    task.status = if gates_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = gates_ok.then_some(completed_at);
    task.evidence = crate::runtime::goal::evidence::local_verification_task_evidence(gates, gates_ok);
    Some(task.clone())
}

pub(crate) async fn append_local_verification_task_events(
    state: &GoalState,
    task: &GoalTask,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let task_id = TaskId(task.id.clone());
    let summary = crate::runtime::goal::planner::controller_task_summary(task);

    let started = Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
        .with_actor(GOAL_CONTROLLER_ACTOR)
        .with_payload(serde_json::json!({
            "task_id": task.id,
            "worker_id": GOAL_CONTROLLER_ACTOR,
            "title": task.title,
        }))?;

    let finished = if task.status == GoalTaskStatus::Done {
        builder.task_completed(task_id, worker_id, Some(&summary))?
    } else {
        Event::new(RunId(state.goal_id.clone()), EventKind::TaskFailed)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": GOAL_CONTROLLER_ACTOR,
                "summary": summary,
            }))?
    };

    writer.append_many(&[started, finished]).await
}

pub(crate) async fn append_gate_events(state: &GoalState, gates: &[GateResult]) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let mut events = Vec::new();

    for gate in gates {
        let gate_id = GateId(gate.name.clone());
        events.push(builder.command_finished(
            gate_id.clone(),
            &gate.name,
            &gate.command_line,
            gate.exit_code,
            gate.timed_out,
            gate.stdout_summary.as_deref(),
            gate.stderr_summary.as_deref(),
            gate.output_path.as_deref(),
        )?);

        let gate_event = if gate.passed {
            builder.gate_passed_with_evidence(
                gate_id,
                &gate.name,
                gate.required,
                Some(&gate.command_line),
                gate.exit_code,
                gate.timed_out,
                gate.stdout_summary.as_deref(),
                gate.stderr_summary.as_deref(),
                gate.output_path.as_deref(),
                Some(gate.timeout_secs),
            )
        } else {
            builder.gate_failed_with_evidence(
                gate_id,
                &gate.name,
                gate.required,
                Some(&gate.command_line),
                gate.exit_code,
                gate.timed_out,
                gate.stdout_summary.as_deref(),
                gate.stderr_summary.as_deref(),
                gate.output_path.as_deref(),
                Some(gate.timeout_secs),
            )
        }?;
        events.push(gate_event);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}
