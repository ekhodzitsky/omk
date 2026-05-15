use super::{goal_agent_task_policy_payload, GoalAgentTaskProposal, PerTaskBudgetSnapshot};

pub fn task_dispatch_accepted_payload(
    proposal: &GoalAgentTaskProposal,
    snapshot: &PerTaskBudgetSnapshot,
) -> anyhow::Result<serde_json::Value> {
    let mut value = goal_agent_task_policy_payload(proposal, Some("accepted by goal policy"));
    if let serde_json::Value::Object(ref mut map) = value {
        map.insert(
            "budget_snapshot".to_string(),
            serde_json::to_value(snapshot)?,
        );
    }
    Ok(value)
}

pub fn task_dispatch_rejected_payload(
    proposal: &GoalAgentTaskProposal,
    reason: &str,
    snapshot: Option<&PerTaskBudgetSnapshot>,
) -> anyhow::Result<serde_json::Value> {
    let mut value = goal_agent_task_policy_payload(proposal, Some(reason));
    if let serde_json::Value::Object(ref mut map) = value {
        if let Some(snapshot) = snapshot {
            map.insert(
                "budget_snapshot".to_string(),
                serde_json::to_value(snapshot)?,
            );
        }
    }
    Ok(value)
}
