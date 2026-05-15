use chrono::{DateTime, Utc};

use super::{GoalAgentTaskProposal, GoalState, GoalTaskGraph, Task, GOAL_LOCAL_VERIFY_TASK_ID};

pub fn goal_agent_scheduler_tasks(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
    controller_task_id: &str,
    proposals: &[GoalAgentTaskProposal],
) -> Vec<Task> {
    proposals
        .iter()
        .map(|proposal| {
            let mut task = Task::new(proposal.id.clone(), proposal.title.clone())
                .with_description(goal_agent_task_prompt(
                    state,
                    task_graph,
                    generated_at,
                    proposal,
                ))
                .with_dependencies(proposal.dependencies.clone())
                .with_read_set(proposal.read_set.clone())
                .with_write_set(proposal.write_set.clone())
                .with_priority(proposal.priority)
                .with_max_retries(0);
            task.extra.insert(
                "acceptance".to_string(),
                serde_json::json!(proposal.acceptance),
            );
            task.extra.insert(
                "budget_secs".to_string(),
                serde_json::json!(proposal.budget_secs),
            );
            task.extra
                .insert("risk".to_string(), serde_json::json!(proposal.risk));
            task.extra.insert(
                "controller_task_id".to_string(),
                serde_json::json!(controller_task_id),
            );
            task
        })
        .collect()
}

pub(super) fn goal_agent_task_prompt(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
    proposal: &GoalAgentTaskProposal,
) -> String {
    let local_status = task_graph
        .tasks
        .iter()
        .find(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID)
        .map(|task| task.status.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "Goal ID: {}\nGenerated: {generated_at}\n\nOriginal goal:\n{}\n\nNormalized goal:\n{}\n\nController task: {}\nTitle: {}\nBudget: {} seconds\nRisk: {}\n\nTask:\n{}\n\nAcceptance criteria:\n- {}\n\nPolicy:\nStay inside the current repository, keep the diff minimal, do not commit, do not publish, do not touch secrets, and summarize changed files plus verification still needed for production readiness.\n\nLocal verification task status: {local_status}",
        state.goal_id,
        state.original_goal,
        state.normalized_goal,
        proposal.id,
        proposal.title,
        proposal.budget_secs,
        proposal.risk,
        proposal.description,
        proposal.acceptance.join("\n- ")
    )
}
