use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use super::metadata::{GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryStatus};
use super::persist::load_goal_task_delivery_records;
use super::persist::update_goal_task_delivery_metadata;
use crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID;
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::goal::worktree::plan_goal_worktree;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalDeliverySlicePlan {
    pub goal_id: String,
    pub slices: Vec<GoalDeliverySlice>,
    pub overlap_serializations: Vec<GoalDeliveryOverlapSerialization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalDeliverySlice {
    pub slice_id: String,
    pub task_id: String,
    pub owner_role: String,
    pub read_scope: Vec<String>,
    pub write_scope: Vec<String>,
    pub dependencies: Vec<String>,
    pub branch_name: String,
    pub worktree_name: String,
    pub worktree_path: PathBuf,
    pub gates: Vec<String>,
    pub review_needs: Vec<String>,
    pub pr_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalDeliveryOverlapSerialization {
    pub blocked_slice_id: String,
    pub serializes_after: String,
    pub kind: String,
    pub path: String,
}

pub fn plan_goal_delivery_slices(
    worktrees_root: impl AsRef<Path>,
    task_graph: &GoalTaskGraph,
) -> Result<GoalDeliverySlicePlan> {
    task_graph.validate()?;
    let delivery_task_ids = delivery_task_ids(task_graph);
    let dependency_closures = dependency_closures(task_graph);
    let tasks_by_id = tasks_by_id(task_graph);
    let mut slices = Vec::new();
    let mut overlap_serializations = Vec::new();

    for task_id in topological_task_ids(task_graph)? {
        if !delivery_task_ids.contains(task_id.as_str()) {
            continue;
        }
        let task = tasks_by_id
            .get(task_id.as_str())
            .copied()
            .with_context(|| {
                format!("task graph lost task id during delivery planning: {task_id}")
            })?;
        let mut dependencies = delivery_dependencies(task, &delivery_task_ids);

        for prior in &slices {
            let Some(overlap) = first_access_overlap(task, prior) else {
                continue;
            };
            let already_ordered = dependency_closures
                .get(task.id.as_str())
                .is_some_and(|dependencies| dependencies.contains(prior.task_id.as_str()));
            if already_ordered {
                continue;
            }
            dependencies.insert(prior.slice_id.clone());
            overlap_serializations.push(GoalDeliveryOverlapSerialization {
                blocked_slice_id: task.id.clone(),
                serializes_after: prior.slice_id.clone(),
                kind: overlap.kind.to_string(),
                path: overlap.path,
            });
        }

        let worktree = plan_goal_worktree(worktrees_root.as_ref(), &task_graph.goal_id, &task.id)?;
        slices.push(GoalDeliverySlice {
            slice_id: task.id.clone(),
            task_id: task.id.clone(),
            owner_role: owner_role_for_task(task),
            read_scope: sorted_unique(task.read_set.iter().cloned()),
            write_scope: sorted_unique(task.write_set.iter().cloned()),
            dependencies: dependencies.into_iter().collect(),
            branch_name: worktree.branch_name,
            worktree_name: worktree.worktree_name,
            worktree_path: worktree.worktree_path,
            gates: gates_for_task(task),
            review_needs: review_needs_for_task(task),
            pr_url: None,
        });
    }

    Ok(GoalDeliverySlicePlan {
        goal_id: task_graph.goal_id.clone(),
        slices,
        overlap_serializations,
    })
}

pub async fn record_goal_delivery_slice_plan(
    goal_dir: &Path,
    plan: &GoalDeliverySlicePlan,
) -> Result<()> {
    for slice in &plan.slices {
        update_goal_task_delivery_metadata(
            goal_dir,
            &slice.task_id,
            GoalTaskDeliveryMetadataUpdate {
                slice_id: Some(slice.slice_id.clone()),
                owner: Some(slice.owner_role.clone()),
                read_scope: Some(slice.read_scope.clone()),
                write_scope: Some(slice.write_scope.clone()),
                dependencies: Some(slice.dependencies.clone()),
                branch: Some(slice.branch_name.clone()),
                worktree_name: Some(slice.worktree_name.clone()),
                worktree_path: Some(slice.worktree_path.clone()),
                gates: Some(slice.gates.clone()),
                review_needs: Some(slice.review_needs.clone()),
                status: Some(GoalTaskDeliveryStatus::Planned),
                ..GoalTaskDeliveryMetadataUpdate::default()
            },
        )
        .await?;
    }
    Ok(())
}

fn delivery_task_ids(task_graph: &GoalTaskGraph) -> BTreeSet<String> {
    task_graph
        .tasks
        .iter()
        .filter(|task| task.status != GoalTaskStatus::Done)
        .filter(|task| !task.write_set.is_empty())
        .filter(|task| task.owner_role.is_some() || writes_project_files(task))
        .map(|task| task.id.clone())
        .collect()
}

fn writes_project_files(task: &GoalTask) -> bool {
    task.id == GOAL_AGENT_EXECUTE_TASK_ID
        || task
            .write_set
            .iter()
            .any(|path| path.trim() == "project files")
}

fn tasks_by_id(task_graph: &GoalTaskGraph) -> HashMap<&str, &GoalTask> {
    task_graph
        .tasks
        .iter()
        .map(|task| (task.id.as_str(), task))
        .collect()
}

fn topological_task_ids(task_graph: &GoalTaskGraph) -> Result<Vec<String>> {
    let tasks_by_id = tasks_by_id(task_graph);
    let mut remaining = task_graph
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::new();
    let mut emitted = HashSet::new();

    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .find(|task_id| {
                tasks_by_id
                    .get(task_id.as_str())
                    .is_some_and(|task| task.dependencies.iter().all(|dep| emitted.contains(dep)))
            })
            .cloned();
        let Some(task_id) = ready else {
            anyhow::bail!("task graph contains a dependency cycle");
        };
        remaining.remove(&task_id);
        emitted.insert(task_id.clone());
        ordered.push(task_id);
    }

    Ok(ordered)
}

fn dependency_closures(task_graph: &GoalTaskGraph) -> HashMap<&str, HashSet<&str>> {
    let tasks_by_id = tasks_by_id(task_graph);
    task_graph
        .tasks
        .iter()
        .map(|task| {
            let mut closure = HashSet::new();
            collect_dependency_closure(task, &tasks_by_id, &mut closure);
            (task.id.as_str(), closure)
        })
        .collect()
}

fn collect_dependency_closure<'a>(
    task: &'a GoalTask,
    tasks_by_id: &HashMap<&'a str, &'a GoalTask>,
    closure: &mut HashSet<&'a str>,
) {
    for dependency in &task.dependencies {
        if !closure.insert(dependency.as_str()) {
            continue;
        }
        if let Some(dependency_task) = tasks_by_id.get(dependency.as_str()) {
            collect_dependency_closure(dependency_task, tasks_by_id, closure);
        }
    }
}

fn delivery_dependencies(
    task: &GoalTask,
    delivery_task_ids: &BTreeSet<String>,
) -> BTreeSet<String> {
    task.dependencies
        .iter()
        .filter(|dependency| delivery_task_ids.contains(dependency.as_str()))
        .cloned()
        .collect()
}

fn owner_role_for_task(task: &GoalTask) -> String {
    task.owner_role
        .clone()
        .unwrap_or_else(|| "executor".to_string())
}

fn gates_for_task(task: &GoalTask) -> Vec<String> {
    let mut gates = BTreeSet::from([
        "acceptance_criteria".to_string(),
        "local_verification_wall".to_string(),
    ]);
    if task.risk == "high" || task.risk.contains("security") {
        gates.insert("security_review_gate".to_string());
    }
    gates.into_iter().collect()
}

fn review_needs_for_task(task: &GoalTask) -> Vec<String> {
    let mut reviews = BTreeSet::from([
        "anti_slop_review".to_string(),
        "code_review".to_string(),
        "test_engineer_review".to_string(),
    ]);
    if task.risk != "low" {
        reviews.insert("architect_review".to_string());
        reviews.insert("security_review".to_string());
    }
    if task
        .write_set
        .iter()
        .chain(task.read_set.iter())
        .any(|path| path.contains("bench") || path.contains("performance"))
    {
        reviews.insert("performance_review".to_string());
    }
    reviews.into_iter().collect()
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct AccessOverlap {
    kind: &'static str,
    path: String,
}

fn first_access_overlap(task: &GoalTask, prior: &GoalDeliverySlice) -> Option<AccessOverlap> {
    first_conflicting_path(&task.write_set, &prior.write_scope)
        .map(|path| AccessOverlap {
            kind: "write_write",
            path,
        })
        .or_else(|| {
            first_conflicting_path(&task.write_set, &prior.read_scope).map(|path| AccessOverlap {
                kind: "write_read",
                path,
            })
        })
        .or_else(|| {
            first_conflicting_path(&task.read_set, &prior.write_scope).map(|path| AccessOverlap {
                kind: "read_write",
                path,
            })
        })
}

fn first_conflicting_path(candidate: &[String], accepted: &[String]) -> Option<String> {
    candidate.iter().find_map(|candidate_path| {
        accepted
            .iter()
            .find(|accepted_path| paths_conflict(candidate_path, accepted_path))
            .map(|_| display_goal_path(candidate_path))
    })
}

fn paths_conflict(candidate: &str, accepted: &str) -> bool {
    let Some(candidate) = normalize_goal_path(candidate) else {
        return false;
    };
    let Some(accepted) = normalize_goal_path(accepted) else {
        return false;
    };

    candidate == "project files"
        || accepted == "project files"
        || candidate == accepted
        || is_path_prefix(&candidate, &accepted)
        || is_path_prefix(&accepted, &candidate)
}

fn display_goal_path(path: &str) -> String {
    normalize_goal_path(path).unwrap_or_else(|| path.trim().to_string())
}

fn normalize_goal_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed == "project files" {
        return Some(trimmed.to_string());
    }

    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!parts.is_empty()).then(|| parts.join("/"))
}

fn is_path_prefix(parent: &str, child: &str) -> bool {
    child
        .strip_prefix(parent)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

/// Returns slices whose task is not Done and whose dependencies (including
/// overlap serializations recorded in delivery metadata) are satisfied.
pub async fn ready_delivery_slices(
    goal_dir: &Path,
    task_graph: &GoalTaskGraph,
) -> Result<Vec<GoalDeliverySlice>> {
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
pub async fn all_slices_done(goal_dir: &Path, task_graph: &GoalTaskGraph) -> Result<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};

    fn task(id: &str, status: GoalTaskStatus, dependencies: &[&str]) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: dependencies.iter().map(|d| d.to_string()).collect(),
            read_set: Vec::new(),
            write_set: vec!["project files".to_string()],
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
        GoalTaskGraph {
            version: 1,
            goal_id: "goal-test".to_string(),
            generated_at: chrono::Utc::now(),
            tasks,
        }
    }

    #[tokio::test]
    async fn all_slices_done_returns_false_when_no_records() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph = graph(vec![task("t1", GoalTaskStatus::Pending, &[])]);
        let task_graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": chrono::Utc::now(),
            "tasks": [
                {
                    "id": "t1",
                    "title": "Task t1",
                    "description": "desc",
                    "status": "pending",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"]
                }
            ]
        });
        tokio::fs::write(
            tmp.path()
                .join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&task_graph_json).expect("json"),
        )
        .await
        .expect("write");

        let done = all_slices_done(tmp.path(), &graph)
            .await
            .expect("all_slices_done");
        assert!(!done, "no delivery records means not done");
    }

    #[tokio::test]
    async fn all_slices_done_returns_true_when_all_tasks_done() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph = graph(vec![
            task("t1", GoalTaskStatus::Done, &[]),
            task("t2", GoalTaskStatus::Done, &["t1"]),
        ]);
        let task_graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": chrono::Utc::now(),
            "tasks": [
                {
                    "id": "t1",
                    "title": "Task t1",
                    "description": "desc",
                    "status": "done",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": {
                        "slice_id": "t1",
                        "worktree_path": "/tmp/wt1",
                        "status": "delivered"
                    }
                },
                {
                    "id": "t2",
                    "title": "Task t2",
                    "description": "desc",
                    "status": "done",
                    "dependencies": ["t1"],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": {
                        "slice_id": "t2",
                        "worktree_path": "/tmp/wt2",
                        "status": "delivered"
                    }
                }
            ]
        });
        tokio::fs::write(
            tmp.path()
                .join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&task_graph_json).expect("json"),
        )
        .await
        .expect("write");

        let done = all_slices_done(tmp.path(), &graph)
            .await
            .expect("all_slices_done");
        assert!(done, "all slice tasks are done");
    }

    #[tokio::test]
    async fn ready_delivery_slices_filters_done_and_blocked_dependencies() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph = graph(vec![
            task("t1", GoalTaskStatus::Done, &[]),
            task("t2", GoalTaskStatus::Pending, &["t1"]),
            task("t3", GoalTaskStatus::Pending, &["t1"]),
        ]);
        let task_graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": chrono::Utc::now(),
            "tasks": [
                {
                    "id": "t1",
                    "title": "Task t1",
                    "description": "desc",
                    "status": "done",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": { "slice_id": "t1", "worktree_path": "/tmp/wt1", "status": "delivered" }
                },
                {
                    "id": "t2",
                    "title": "Task t2",
                    "description": "desc",
                    "status": "pending",
                    "dependencies": ["t1"],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": { "slice_id": "t2", "worktree_path": "/tmp/wt2", "status": "planned", "dependencies": ["t1"] }
                },
                {
                    "id": "t3",
                    "title": "Task t3",
                    "description": "desc",
                    "status": "pending",
                    "dependencies": ["t1"],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": { "slice_id": "t3", "worktree_path": "/tmp/wt3", "status": "planned", "dependencies": ["t1", "t2"] }
                }
            ]
        });
        tokio::fs::write(
            tmp.path()
                .join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&task_graph_json).expect("json"),
        )
        .await
        .expect("write");

        let ready = ready_delivery_slices(tmp.path(), &graph)
            .await
            .expect("ready");
        assert_eq!(ready.len(), 1, "only t2 is ready (t3 blocked on t2)");
        assert_eq!(ready[0].task_id, "t2");
        assert_eq!(ready[0].worktree_path, PathBuf::from("/tmp/wt2"));
    }
}
