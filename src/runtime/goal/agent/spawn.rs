use crate::runtime::goal::agent::types::{
    GoalAgentDispatchPlan, GoalAgentTaskProposal, GoalAgentWaveKind,
};
use crate::runtime::goal::state::{
    GoalState, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_AGENT_FOLLOWUPS_RUN_ID, GOAL_AGENT_VERIFY_TASK_ID,
};
use crate::runtime::goal::task_graph::{
    goal_task_done, pending_goal_agent_followup_proposals, GoalTaskGraph, GoalTaskStatus,
};

pub(crate) fn goal_agent_dispatch_plan(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
) -> Option<GoalAgentDispatchPlan> {
    if state.slice_execution {
        return goal_agent_slice_dispatch_plan(state, task_graph, "");
    }
    goal_agent_dispatch_plan_with_run_key(state, task_graph, GOAL_AGENT_EXECUTE_TASK_ID)
}

/// Build a dispatch plan for a specific slice, using a slice-specific run key
/// so concurrent slice waves do not collide on worker directories.
///
/// When `slice_task_id` is empty, this finds the first not-done implement task
/// (or verify task if all implements are done) for serial dispatch.
pub(crate) fn goal_agent_slice_dispatch_plan(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    slice_task_id: &str,
) -> Option<GoalAgentDispatchPlan> {
    // If a specific slice task id is provided, return its proposal directly.
    if !slice_task_id.is_empty() {
        let task = task_graph.tasks.iter().find(|t| t.id == slice_task_id)?;
        let run_key = if slice_task_id.starts_with("goal-agent-implement-") {
            slice_task_id.to_string()
        } else {
            format!("{}-{}", GOAL_AGENT_EXECUTE_TASK_ID, slice_task_id)
        };
        return Some(GoalAgentDispatchPlan {
            run_key,
            kind: GoalAgentWaveKind::Initial,
            proposals: vec![super::proposal_from_task(state, task)],
            allow_existing_task_ids: true,
        });
    }

    // Serial slice dispatch: find first not-done implement task.
    for task in &task_graph.tasks {
        if task.id.starts_with("goal-agent-implement-") && task.status != GoalTaskStatus::Done {
            return Some(GoalAgentDispatchPlan {
                run_key: task.id.clone(),
                kind: GoalAgentWaveKind::Initial,
                proposals: vec![super::proposal_from_task(state, task)],
                allow_existing_task_ids: true,
            });
        }
    }

    // All implements done — dispatch verify if not done.
    if let Some(verify_task) = task_graph
        .tasks
        .iter()
        .find(|t| t.id == GOAL_AGENT_VERIFY_TASK_ID && t.status != GoalTaskStatus::Done)
    {
        return Some(GoalAgentDispatchPlan {
            run_key: GOAL_AGENT_VERIFY_TASK_ID.to_string(),
            kind: GoalAgentWaveKind::Initial,
            proposals: vec![super::proposal_from_task(state, verify_task)],
            allow_existing_task_ids: true,
        });
    }

    // Follow-ups
    let proposals = pending_goal_agent_followup_proposals(task_graph);
    (!proposals.is_empty()).then(|| GoalAgentDispatchPlan {
        run_key: GOAL_AGENT_FOLLOWUPS_RUN_ID.to_string(),
        kind: GoalAgentWaveKind::FollowUp,
        proposals,
        allow_existing_task_ids: true,
    })
}

fn goal_agent_dispatch_plan_with_run_key(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    run_key: &str,
) -> Option<GoalAgentDispatchPlan> {
    if !goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID) {
        return Some(GoalAgentDispatchPlan {
            run_key: run_key.to_string(),
            kind: GoalAgentWaveKind::Initial,
            proposals: super::propose_goal_agent_tasks(state),
            allow_existing_task_ids: false,
        });
    }

    let proposals = pending_goal_agent_followup_proposals(task_graph);
    (!proposals.is_empty()).then(|| GoalAgentDispatchPlan {
        // Use the original follow-up run key when this is the default plan;
        // only suffix for slice-specific plans so worker directories stay
        // predictable for existing tests and non-slice callers.
        run_key: if run_key == GOAL_AGENT_EXECUTE_TASK_ID {
            GOAL_AGENT_FOLLOWUPS_RUN_ID.to_string()
        } else {
            format!("{}-{}", GOAL_AGENT_FOLLOWUPS_RUN_ID, run_key)
        },
        kind: GoalAgentWaveKind::FollowUp,
        proposals,
        allow_existing_task_ids: true,
    })
}

pub(crate) fn proposal_from_task(
    state: &GoalState,
    task: &crate::runtime::goal::task_graph::GoalTask,
) -> GoalAgentTaskProposal {
    GoalAgentTaskProposal {
        id: task.id.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        dependencies: task.dependencies.clone(),
        read_set: task.read_set.clone(),
        write_set: task.write_set.clone(),
        risk: task.risk.clone(),
        acceptance: task.acceptance.clone(),
        budget_secs: crate::runtime::goal::state::goal_agent_task_budget_secs(state, 900),
        priority: if task.id.starts_with("goal-agent-implement-") {
            20
        } else {
            10
        },
    }
}
