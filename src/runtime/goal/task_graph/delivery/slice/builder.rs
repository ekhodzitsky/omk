use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path};

use anyhow::Context;

use crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID;
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::goal::task_graph::delivery::metadata::{GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryStatus};
use crate::runtime::goal::task_graph::delivery::persist::update_goal_task_delivery_metadata;
use crate::runtime::goal::worktree::plan_goal_worktree;
use super::types::{AccessOverlap, GoalDeliveryOverlapSerialization, GoalDeliverySlice, GoalDeliverySlicePlan};

pub fn plan_goal_delivery_slices(
    worktrees_root: impl AsRef<Path>,
    task_graph: &GoalTaskGraph,
) -> anyhow::Result<GoalDeliverySlicePlan> {
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
) -> anyhow::Result<()> {
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

fn topological_task_ids(task_graph: &GoalTaskGraph) -> anyhow::Result<Vec<String>> {
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
