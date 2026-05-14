use anyhow::Result;
use serde_json::Value;
use std::path::Path;

use super::metadata::{GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryStatus};
use super::persist::{
    load_task_graph_value, task_graph_path, task_ids_in_value, update_goal_task_delivery_metadata,
};
use crate::runtime::goal::worktree::GoalWorktreePlan;

pub(crate) async fn ensure_worktree_delivery_targets(
    goal_dir: &Path,
    plans: &[GoalWorktreePlan],
) -> Result<()> {
    let value = load_task_graph_value(goal_dir).await?;
    let task_ids = task_ids_in_value(&value);
    for plan in plans {
        if !task_ids.contains(plan.task_id.as_str()) {
            anyhow::bail!(
                "cannot record goal worktree delivery metadata: task {} not found in {}",
                plan.task_id,
                task_graph_path(goal_dir).display()
            );
        }
    }
    Ok(())
}

pub(crate) async fn record_worktree_delivery_metadata(
    goal_dir: &Path,
    plan: &GoalWorktreePlan,
) -> Result<()> {
    let graph = load_task_graph_value(goal_dir).await?;
    let task = graph
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|task| task.get("id").and_then(Value::as_str) == Some(plan.task_id.as_str()));
    let owner = task
        .and_then(|task| task.get("owner_role").and_then(Value::as_str))
        .map(str::to_string);
    let write_scope = task
        .and_then(|task| task.get("write_set"))
        .and_then(|value| serde_json::from_value(value.clone()).ok());

    update_goal_task_delivery_metadata(
        goal_dir,
        &plan.task_id,
        GoalTaskDeliveryMetadataUpdate {
            owner,
            write_scope,
            branch: Some(plan.branch_name.clone()),
            worktree_path: Some(plan.worktree_path.clone()),
            status: Some(GoalTaskDeliveryStatus::Planned),
            ..GoalTaskDeliveryMetadataUpdate::default()
        },
    )
    .await?;
    Ok(())
}
