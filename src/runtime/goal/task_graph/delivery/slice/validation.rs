use std::collections::HashMap;
use std::path::Path;

use super::types::GoalDeliverySlice;
use crate::runtime::goal::task_graph::delivery::persist::load_goal_task_delivery_records;
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};

/// Returns slices whose task is not Done and whose dependencies (including
/// overlap serializations recorded in delivery metadata) are satisfied.
pub async fn ready_delivery_slices(
    goal_dir: &Path,
    task_graph: &GoalTaskGraph,
) -> anyhow::Result<Vec<GoalDeliverySlice>> {
    let records = load_goal_task_delivery_records(goal_dir).await?;
    if records.is_empty() {
        return Ok(Vec::new());
    }

    let tasks_by_id: HashMap<&str, &GoalTask> = task_graph
        .tasks
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();

    let mut ready = Vec::new();
    for record in records {
        let task = match tasks_by_id.get(record.task_id.as_str()) {
            Some(t) => t,
            None => continue,
        };
        if task.status == GoalTaskStatus::Done {
            continue;
        }

        // Check slice-level dependencies (includes overlap serializations)
        let slice_deps_satisfied = record.metadata.dependencies.iter().all(|dep_id| {
            tasks_by_id
                .get(dep_id.as_str())
                .is_some_and(|t| t.status == GoalTaskStatus::Done)
        });
        if !slice_deps_satisfied {
            continue;
        }

        // Also check task-level dependencies
        let task_deps_satisfied = task.dependencies.iter().all(|dep_id| {
            tasks_by_id
                .get(dep_id.as_str())
                .is_some_and(|t| t.status == GoalTaskStatus::Done)
        });
        if !task_deps_satisfied {
            continue;
        }

        ready.push(GoalDeliverySlice {
            slice_id: record
                .metadata
                .slice_id
                .unwrap_or_else(|| record.task_id.clone()),
            task_id: record.task_id,
            owner_role: record.metadata.owner.unwrap_or_default(),
            read_scope: record.metadata.read_scope,
            write_scope: record.metadata.write_scope,
            dependencies: record.metadata.dependencies,
            branch_name: record.metadata.branch.unwrap_or_default(),
            worktree_name: record.metadata.worktree_name.unwrap_or_default(),
            worktree_path: record.metadata.worktree_path.unwrap_or_default(),
            gates: record.metadata.gates,
            review_needs: record.metadata.review_needs,
            pr_url: record.metadata.pr_url,
        });
    }

    Ok(ready)
}

/// True when every delivery slice's task is Done. Returns false when no
/// delivery slices exist (caller should fall back to traditional completion checks).
pub async fn all_slices_done(goal_dir: &Path, task_graph: &GoalTaskGraph) -> anyhow::Result<bool> {
    let records = load_goal_task_delivery_records(goal_dir).await?;
    if records.is_empty() {
        return Ok(false);
    }
    let tasks_by_id: HashMap<&str, &GoalTask> = task_graph
        .tasks
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();
    Ok(records.iter().all(|record| {
        tasks_by_id
            .get(record.task_id.as_str())
            .is_some_and(|task| task.status == GoalTaskStatus::Done)
    }))
}
