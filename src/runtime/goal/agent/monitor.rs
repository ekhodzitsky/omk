use std::collections::{HashMap, HashSet};

use crate::runtime::goal::agent::types::{
    AcceptedGoalAgentAccessSet, GoalAgentAccessConflict, GoalAgentRejectedTask,
    GoalAgentTaskPolicy, GoalAgentTaskProposal,
};
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalTaskGraph, GoalTaskStatus};

pub(crate) fn validate_goal_agent_task_proposals(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    run_id: &str,
    proposals: Vec<GoalAgentTaskProposal>,
    allow_existing_task_ids: bool,
) -> GoalAgentTaskPolicy {
    let existing_task_ids: HashSet<String> = task_graph
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect();
    let mut known_dependencies: HashSet<String> = task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Done)
        .map(|task| task.id.clone())
        .collect();
    let mut accepted_dependency_closures: HashMap<String, HashSet<String>> = HashMap::new();
    let mut accepted_access_sets: Vec<AcceptedGoalAgentAccessSet> = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut accepted_tasks = Vec::new();
    let mut rejected_tasks = Vec::new();

    for proposal in proposals.iter().cloned() {
        let dependency_closure =
            proposal_dependency_closure(&proposal, &accepted_dependency_closures);
        if let Some(reason) = reject_goal_agent_task_proposal(
            &proposal,
            &mut seen_ids,
            &known_dependencies,
            &existing_task_ids,
            &accepted_access_sets,
            &dependency_closure,
            allow_existing_task_ids,
        ) {
            rejected_tasks.push(GoalAgentRejectedTask {
                task: proposal,
                reason,
            });
            continue;
        }

        known_dependencies.insert(proposal.id.clone());
        accepted_access_sets.push(AcceptedGoalAgentAccessSet {
            task_id: proposal.id.clone(),
            read_set: proposal.read_set.clone(),
            write_set: proposal.write_set.clone(),
        });
        accepted_dependency_closures.insert(proposal.id.clone(), dependency_closure);
        accepted_tasks.push(proposal);
    }

    GoalAgentTaskPolicy {
        goal_id: state.goal_id.clone(),
        run_id: run_id.to_string(),
        max_agents: state.max_agents.unwrap_or(1).max(1),
        proposed_tasks: proposals,
        accepted_tasks,
        rejected_tasks,
    }
}

fn reject_goal_agent_task_proposal(
    proposal: &GoalAgentTaskProposal,
    seen_ids: &mut HashSet<String>,
    known_dependencies: &HashSet<String>,
    existing_task_ids: &HashSet<String>,
    accepted_access_sets: &[AcceptedGoalAgentAccessSet],
    dependency_closure: &HashSet<String>,
    allow_existing_task_ids: bool,
) -> Option<String> {
    if proposal.id.trim().is_empty() {
        return Some("task id must not be empty".to_string());
    }
    if proposal.title.trim().is_empty() {
        return Some("task title must not be empty".to_string());
    }
    if proposal.description.trim().is_empty() {
        return Some("task description must not be empty".to_string());
    }
    if !seen_ids.insert(proposal.id.clone()) {
        return Some("duplicate task id rejected by goal policy".to_string());
    }
    if !allow_existing_task_ids && existing_task_ids.contains(&proposal.id) {
        return Some("task id already exists in the goal graph".to_string());
    }
    if proposal.budget_secs == 0 {
        return Some("task budget must be greater than zero".to_string());
    }

    let policy_text = format!(
        "{} {} {}",
        proposal.id,
        proposal.description,
        proposal.write_set.join(" ")
    )
    .to_ascii_lowercase();
    if policy_text.contains("crates.io") || policy_text.contains("publish") {
        return Some("publishing is disabled for GitHub-only goal execution".to_string());
    }

    if let Some(path) = proposal
        .read_set
        .iter()
        .chain(proposal.write_set.iter())
        .find(|path| !crate::runtime::goal::state::is_safe_goal_agent_path(path))
    {
        return Some(format!(
            "path is outside the allowed goal policy roots: {path}"
        ));
    }

    if let Some(dep) = proposal
        .dependencies
        .iter()
        .find(|dep| !known_dependencies.contains(*dep))
    {
        return Some(format!("dependency is not accepted or completed: {dep}"));
    }

    if let Some((task_id, conflict)) =
        first_unordered_access_conflict(proposal, accepted_access_sets, dependency_closure)
    {
        let GoalAgentAccessConflict { kind, path } = conflict;
        return Some(format!("{kind} with accepted task {task_id}: {path}"));
    }

    None
}

fn proposal_dependency_closure(
    proposal: &GoalAgentTaskProposal,
    accepted_dependency_closures: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut closure = HashSet::new();
    for dependency in &proposal.dependencies {
        closure.insert(dependency.clone());
        if let Some(upstream) = accepted_dependency_closures.get(dependency) {
            closure.extend(upstream.iter().cloned());
        }
    }
    closure
}

fn first_unordered_access_conflict(
    proposal: &GoalAgentTaskProposal,
    accepted_access_sets: &[AcceptedGoalAgentAccessSet],
    dependency_closure: &HashSet<String>,
) -> Option<(String, GoalAgentAccessConflict)> {
    for accepted in accepted_access_sets {
        if dependency_closure.contains(&accepted.task_id) {
            continue;
        }
        if let Some(path) = crate::runtime::goal::agent::path_policy::first_conflicting_path(
            &proposal.write_set,
            &accepted.write_set,
        ) {
            return Some((
                accepted.task_id.clone(),
                GoalAgentAccessConflict {
                    kind: "write-set conflict",
                    path,
                },
            ));
        }
        if let Some(path) = crate::runtime::goal::agent::path_policy::first_conflicting_path(
            &proposal.read_set,
            &accepted.write_set,
        ) {
            return Some((
                accepted.task_id.clone(),
                GoalAgentAccessConflict {
                    kind: "read/write conflict",
                    path,
                },
            ));
        }
        if let Some(path) = crate::runtime::goal::agent::path_policy::first_conflicting_path(
            &proposal.write_set,
            &accepted.read_set,
        ) {
            return Some((
                accepted.task_id.clone(),
                GoalAgentAccessConflict {
                    kind: "write/read conflict",
                    path,
                },
            ));
        }
    }
    None
}

pub(crate) fn goal_agent_task_policy_payload(
    proposal: &GoalAgentTaskProposal,
    reason: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "task_id": proposal.id,
        "title": proposal.title,
        "risk": proposal.risk,
        "budget_secs": proposal.budget_secs,
        "dependencies": proposal.dependencies,
        "read_set": proposal.read_set,
        "write_set": proposal.write_set,
        "reason": reason,
    })
}
