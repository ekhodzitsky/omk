use super::state::{
    goal_agent_task_budget_secs, GoalState, GOAL_AGENT_EXECUTE_TASK_ID,
    GOAL_AGENT_FOLLOWUPS_RUN_ID, GOAL_AGENT_IMPLEMENT_TASK_ID, GOAL_AGENT_PUBLISH_TASK_ID,
    GOAL_AGENT_VERIFY_TASK_ID, GOAL_ARTIFACTS_DIR, GOAL_PRD_FILE, GOAL_PROOF_FILE,
    GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
use super::task_graph::{
    goal_task_done, pending_goal_agent_followup_proposals, GoalTaskGraph, GoalTaskStatus,
};
use path_policy::first_conflicting_path;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

mod path_policy;
pub use path_policy::check_task_path_policy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAgentTaskProposal {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub read_set: Vec<String>,
    #[serde(default)]
    pub write_set: Vec<String>,
    #[serde(default = "default_goal_agent_task_risk")]
    pub risk: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default = "default_goal_agent_task_budget_secs")]
    pub budget_secs: u64,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalAgentRejectedTask {
    pub(crate) task: GoalAgentTaskProposal,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalAgentTaskPolicy {
    pub(crate) goal_id: String,
    pub(crate) run_id: String,
    pub(crate) max_agents: usize,
    pub(crate) proposed_tasks: Vec<GoalAgentTaskProposal>,
    pub(crate) accepted_tasks: Vec<GoalAgentTaskProposal>,
    pub(crate) rejected_tasks: Vec<GoalAgentRejectedTask>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalAgentWaveKind {
    Initial,
    FollowUp,
}

#[derive(Debug, Clone)]
pub struct GoalAgentDispatchPlan {
    pub(crate) run_key: String,
    pub(crate) kind: GoalAgentWaveKind,
    pub(crate) proposals: Vec<GoalAgentTaskProposal>,
    pub(crate) allow_existing_task_ids: bool,
}

#[derive(Debug, Clone)]
struct AcceptedGoalAgentAccessSet {
    task_id: String,
    read_set: Vec<String>,
    write_set: Vec<String>,
}

#[derive(Debug, Clone)]
struct GoalAgentAccessConflict {
    kind: &'static str,
    path: String,
}

fn default_goal_agent_task_risk() -> String {
    "moderate".to_string()
}

fn default_goal_agent_task_budget_secs() -> u64 {
    300
}

pub(crate) fn propose_goal_agent_tasks(state: &GoalState) -> Vec<GoalAgentTaskProposal> {
    vec![
        GoalAgentTaskProposal {
            id: GOAL_AGENT_IMPLEMENT_TASK_ID.to_string(),
            title: "Implement bounded goal slice".to_string(),
            description: "Make one bounded in-repository project change that moves the goal forward, then summarize changed files and remaining verification needs.".to_string(),
            dependencies: Vec::new(),
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
            ],
            write_set: vec![
                "project files".to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                GOAL_PROOF_FILE.to_string(),
            ],
            risk: "moderate".to_string(),
            acceptance: vec![
                "Make only a bounded project change needed to move the goal forward.".to_string(),
                "Do not commit, publish, or touch secrets.".to_string(),
                "Summarize changed files and verification still needed for production readiness.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 900),
            priority: 20,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_VERIFY_TASK_ID.to_string(),
            title: "Verify agent wave follow-up".to_string(),
            description: "Inspect the bounded implementation result and summarize verification, review, or hardening follow-up that still blocks readiness.".to_string(),
            dependencies: vec![GOAL_AGENT_IMPLEMENT_TASK_ID.to_string()],
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                "project files".to_string(),
            ],
            write_set: vec![GOAL_ARTIFACTS_DIR.to_string()],
            risk: "low".to_string(),
            acceptance: vec![
                "Review the bounded project change and call out remaining verification gaps.".to_string(),
                "Do not make broad follow-up mutations without a new controller-approved task.".to_string(),
                "Keep the goal proof honest when production readiness is still blocked.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 10,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_PUBLISH_TASK_ID.to_string(),
            title: "Publish crate release".to_string(),
            description: "Publish this crate to crates.io after the agent wave succeeds.".to_string(),
            dependencies: vec![GOAL_AGENT_VERIFY_TASK_ID.to_string()],
            read_set: vec![GOAL_PROOF_FILE.to_string()],
            write_set: vec!["crates.io".to_string()],
            risk: "external-side-effect".to_string(),
            acceptance: vec!["Publish the package to crates.io.".to_string()],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 0,
        },
    ]
}

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
            proposals: vec![proposal_from_task(state, task)],
            allow_existing_task_ids: true,
        });
    }

    // Serial slice dispatch: find first not-done implement task.
    for task in &task_graph.tasks {
        if task.id.starts_with("goal-agent-implement-") && task.status != GoalTaskStatus::Done {
            return Some(GoalAgentDispatchPlan {
                run_key: task.id.clone(),
                kind: GoalAgentWaveKind::Initial,
                proposals: vec![proposal_from_task(state, task)],
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
            proposals: vec![proposal_from_task(state, verify_task)],
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
            proposals: propose_goal_agent_tasks(state),
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

fn proposal_from_task(
    state: &GoalState,
    task: &super::task_graph::GoalTask,
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
        budget_secs: goal_agent_task_budget_secs(state, 900),
        priority: if task.id.starts_with("goal-agent-implement-") {
            20
        } else {
            10
        },
    }
}

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
        .find(|path| !super::state::is_safe_goal_agent_path(path))
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
        if let Some(path) = first_conflicting_path(&proposal.write_set, &accepted.write_set) {
            return Some((
                accepted.task_id.clone(),
                GoalAgentAccessConflict {
                    kind: "write-set conflict",
                    path,
                },
            ));
        }
        if let Some(path) = first_conflicting_path(&proposal.read_set, &accepted.write_set) {
            return Some((
                accepted.task_id.clone(),
                GoalAgentAccessConflict {
                    kind: "read/write conflict",
                    path,
                },
            ));
        }
        if let Some(path) = first_conflicting_path(&proposal.write_set, &accepted.read_set) {
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

#[cfg(test)]
mod tests;
