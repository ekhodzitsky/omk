use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::runtime::goal::review::slop::SlopFinding;
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
    let description = format!("Auto-cleanup task generated from review feedback:\n\n{feedback}");
    upsert_followup_task(
        task_graph,
        slice_task_id,
        "cleanup",
        &description,
        &[
            "Address all review feedback items",
            "Re-run verification gates after cleanup",
        ],
        changed_files,
        generated_at,
    )
}

/// Append a refactor task to the task graph that targets specific slop findings.
/// Wires the original slice task to depend on it. Returns the refactor task id
/// when a new task was created or updated, or `None` if the task graph already
/// contains an identical refactor task for this slice.
pub(crate) fn spawn_refactor_task_from_slop_findings(
    task_graph: &mut GoalTaskGraph,
    slice_task_id: &str,
    findings: &[SlopFinding],
    changed_files: &[String],
    generated_at: DateTime<Utc>,
) -> Option<String> {
    if findings.is_empty() {
        return None;
    }

    let description = build_slop_task_description(findings);
    upsert_followup_task(
        task_graph,
        slice_task_id,
        "refactor",
        &description,
        &[
            "Fix all rough edges identified by anti-slop review",
            "Re-run verification gates after refactoring",
        ],
        changed_files,
        generated_at,
    )
}

fn build_slop_task_description(findings: &[SlopFinding]) -> String {
    let mut lines =
        vec!["Auto-refactor task generated from anti-slop review findings:".to_string()];
    for finding in findings {
        let location = finding
            .line
            .map(|l| format!("line {}", l))
            .unwrap_or_else(|| "file level".to_string());
        lines.push(format!(
            "- [{}] {} at {}: {}",
            finding.kind,
            finding.file.display(),
            location,
            finding.message
        ));
    }
    lines.join("\n")
}

/// Shared mechanics for spawning or updating a follow-up task (cleanup or refactor)
/// and wiring the original slice task to depend on it.
fn upsert_followup_task(
    task_graph: &mut GoalTaskGraph,
    slice_task_id: &str,
    kind: &str,
    description: &str,
    acceptance: &[&str],
    changed_files: &[String],
    generated_at: DateTime<Utc>,
) -> Option<String> {
    let task_id = format!("goal-agent-{kind}-{slice_task_id}");

    if let Some(existing) = task_graph.tasks.iter_mut().find(|task| task.id == task_id) {
        if existing.description == description {
            return None;
        }
        existing.description = description.to_string();
        existing.status = GoalTaskStatus::Pending;
        existing.completed_at = None;
        existing.read_set = changed_files.to_vec();
        existing.write_set = changed_files.to_vec();
        existing.evidence.push(GoalTaskEvidence {
            kind: format!("{kind}_update"),
            path: PathBuf::new(),
            summary: format!(
                "{} task updated at {generated_at} for slice {slice_task_id}",
                first_char_uppercase(kind)
            ),
        });
        return Some(task_id);
    }

    let task = GoalTask {
        id: task_id.clone(),
        title: format!("{} slice {}", first_char_uppercase(kind), slice_task_id),
        description: description.to_string(),
        status: GoalTaskStatus::Pending,
        owner_role: Some(GOAL_AGENT_WORKER_ROLE.to_string()),
        completed_at: None,
        evidence: vec![GoalTaskEvidence {
            kind: format!("{kind}_proposal"),
            path: PathBuf::new(),
            summary: format!(
                "{} task spawned at {generated_at} for slice {slice_task_id}",
                first_char_uppercase(kind)
            ),
        }],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: Vec::new(),
        read_set: changed_files.to_vec(),
        write_set: changed_files.to_vec(),
        risk: "low".to_string(),
        acceptance: acceptance.iter().map(|s| s.to_string()).collect(),
    };
    task_graph.tasks.push(task);

    if let Some(slice_task) = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == slice_task_id)
    {
        if !slice_task.dependencies.contains(&task_id) {
            slice_task.dependencies.push(task_id.clone());
        }
        slice_task.status = GoalTaskStatus::Pending;
        slice_task.completed_at = None;
    }

    Some(task_id)
}

fn first_char_uppercase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
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
