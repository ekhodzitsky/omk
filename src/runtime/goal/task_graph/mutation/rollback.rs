use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::runtime::goal::state::GOAL_AGENT_WORKER_ROLE;
use crate::runtime::goal::task_graph::model::{
    GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus,
};

/// Append a cleanup task to the task graph and wire the original slice task
/// to depend on it. Returns the cleanup task id when a new task was created
/// or updated, or `None` if the task graph already contains an identical
/// cleanup task for this slice.
pub(crate) fn spawn_cleanup_task(
    task_graph: &mut GoalTaskGraph,
    slice_task_id: &str,
    feedback: &str,
    changed_files: &[String],
    generated_at: DateTime<Utc>,
) -> Option<String> {
    let cleanup_task_id = format!("goal-agent-cleanup-{slice_task_id}");
    if let Some(existing) = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == cleanup_task_id)
    {
        // Update existing cleanup task with new feedback if it differs.
        let new_description =
            format!("Auto-cleanup task generated from review feedback:\n\n{feedback}");
        if existing.description == new_description {
            return None;
        }
        existing.description = new_description;
        existing.status = GoalTaskStatus::Pending;
        existing.completed_at = None;
        existing.read_set = changed_files.to_vec();
        existing.write_set = changed_files.to_vec();
        existing.evidence.push(GoalTaskEvidence {
            kind: "cleanup_update".to_string(),
            path: PathBuf::new(),
            summary: format!("Cleanup task updated at {generated_at} for slice {slice_task_id}"),
        });
        return Some(cleanup_task_id);
    }
    let cleanup_task = GoalTask {
        id: cleanup_task_id.clone(),
        title: format!("Cleanup slice {slice_task_id}"),
        description: format!("Auto-cleanup task generated from review feedback:\n\n{feedback}"),
        status: GoalTaskStatus::Pending,
        owner_role: Some(GOAL_AGENT_WORKER_ROLE.to_string()),
        completed_at: None,
        evidence: vec![GoalTaskEvidence {
            kind: "cleanup_proposal".to_string(),
            path: PathBuf::new(),
            summary: format!("Cleanup task spawned at {generated_at} for slice {slice_task_id}"),
        }],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: Vec::new(),
        read_set: changed_files.to_vec(),
        write_set: changed_files.to_vec(),
        risk: "low".to_string(),
        acceptance: vec![
            "Address all review feedback items".to_string(),
            "Re-run verification gates after cleanup".to_string(),
        ],
    };
    task_graph.tasks.push(cleanup_task);
    if let Some(task) = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == slice_task_id)
    {
        if !task.dependencies.contains(&cleanup_task_id) {
            task.dependencies.push(cleanup_task_id.clone());
        }
        task.status = GoalTaskStatus::Pending;
        task.completed_at = None;
    }
    Some(cleanup_task_id)
}

/// Merge task graph deltas produced by concurrent slice post-processing
/// back into the main task graph. Assumes slices are non-conflicting
/// (i.e. they do not write to overlapping tasks), but defensively
/// deduplicates new tasks and merges evidence.
pub(crate) fn merge_concurrent_slice_task_graphs(
    main: &mut GoalTaskGraph,
    deltas: &[GoalTaskGraph],
) {
    use std::collections::HashMap;

    // Collect new tasks (by id) from all deltas
    let mut new_tasks_by_id: HashMap<String, GoalTask> = HashMap::new();
    for delta in deltas {
        for task in &delta.tasks {
            if !main.tasks.iter().any(|t| t.id == task.id) {
                new_tasks_by_id
                    .entry(task.id.clone())
                    .or_insert_with(|| task.clone());
            }
        }
    }
    for task in new_tasks_by_id.into_values() {
        main.tasks.push(task);
    }

    // Update existing tasks with the most advanced status and merged evidence
    for task in main.tasks.iter_mut() {
        for delta in deltas {
            if let Some(dt) = delta.tasks.iter().find(|t| t.id == task.id) {
                let precedence = |s: GoalTaskStatus| match s {
                    GoalTaskStatus::Done => 2,
                    GoalTaskStatus::Blocked => 1,
                    GoalTaskStatus::Pending => 0,
                };
                if precedence(dt.status) > precedence(task.status) {
                    task.status = dt.status;
                    task.completed_at = dt.completed_at;
                    task.owner_role = dt.owner_role.clone();
                }
                for ev in &dt.evidence {
                    if !task
                        .evidence
                        .iter()
                        .any(|e| e.kind == ev.kind && e.path == ev.path && e.summary == ev.summary)
                    {
                        task.evidence.push(ev.clone());
                    }
                }
                task.retry_count = dt.retry_count;
                task.lease_expires_at = dt.lease_expires_at;
                for dep in &dt.dependencies {
                    if !task.dependencies.contains(dep) {
                        task.dependencies.push(dep.clone());
                    }
                }
            }
        }
    }
}
