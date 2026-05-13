use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::agent::GoalAgentTaskPolicy;
use super::evidence::GoalAgentRunEvidence;
use super::proof::write_json_artifact;
use super::state::{GoalState, GOAL_AGENT_WORKER_ROLE, GOAL_TASK_GRAPH_FILE};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalTaskStatus {
    Pending,
    Blocked,
    Done,
}

impl std::fmt::Display for GoalTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalTaskStatus::Pending => "pending",
            GoalTaskStatus::Blocked => "blocked",
            GoalTaskStatus::Done => "done",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskEvidence {
    pub kind: String,
    pub path: PathBuf,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: GoalTaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub evidence: Vec<GoalTaskEvidence>,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub dependencies: Vec<String>,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub risk: String,
    pub acceptance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskGraph {
    pub version: u32,
    pub goal_id: String,
    pub generated_at: DateTime<Utc>,
    pub tasks: Vec<GoalTask>,
}

impl GoalTaskGraph {
    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
        let graph: Self = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
        graph
            .validate()
            .with_context(|| format!("Invalid goal task graph: {}", path.display()))?;
        Ok(graph)
    }

    pub fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        if self.version == 0 {
            errors.push("task graph version must be greater than zero".to_string());
        }
        if self.goal_id.trim().is_empty() {
            errors.push("task graph goal_id must not be empty".to_string());
        }
        if self.tasks.is_empty() {
            errors.push("task graph must contain at least one task".to_string());
        }

        let mut task_ids = HashSet::new();
        for task in &self.tasks {
            let task_id = task.id.trim();
            if task_id.is_empty() {
                errors.push("task id must not be empty".to_string());
                continue;
            }
            if !task_ids.insert(task.id.as_str()) {
                errors.push(format!("duplicate task id: {}", task.id));
            }
            if task.title.trim().is_empty() {
                errors.push(format!("task {} title must not be empty", task.id));
            }
            if task.description.trim().is_empty() {
                errors.push(format!("task {} description must not be empty", task.id));
            }
            if task.acceptance.is_empty() {
                errors.push(format!(
                    "task {} must define at least one acceptance criterion",
                    task.id
                ));
            }
        }

        for task in &self.tasks {
            for dependency in &task.dependencies {
                if dependency == &task.id {
                    errors.push(format!("task {} cannot depend on itself", task.id));
                } else if !task_ids.contains(dependency.as_str()) {
                    errors.push(format!(
                        "task {} depends on missing task {}",
                        task.id, dependency
                    ));
                }
            }
        }

        if self.contains_dependency_cycle() {
            errors.push("task graph contains a dependency cycle".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }

    fn contains_dependency_cycle(&self) -> bool {
        let tasks_by_id: HashMap<&str, &GoalTask> = self
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task))
            .collect();
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        self.tasks.iter().any(|task| {
            dependency_cycle_from(task.id.as_str(), &tasks_by_id, &mut visiting, &mut visited)
        })
    }
}

fn dependency_cycle_from(
    task_id: &str,
    tasks_by_id: &HashMap<&str, &GoalTask>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if visited.contains(task_id) {
        return false;
    }
    if !visiting.insert(task_id.to_string()) {
        return true;
    }

    if let Some(task) = tasks_by_id.get(task_id) {
        for dependency in &task.dependencies {
            if tasks_by_id.contains_key(dependency.as_str())
                && dependency_cycle_from(dependency, tasks_by_id, visiting, visited)
            {
                return true;
            }
        }
    }

    visiting.remove(task_id);
    visited.insert(task_id.to_string());
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskGraphSummary {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub blocked_tasks: usize,
    pub done_tasks: usize,
}

pub(crate) fn goal_task_done(task_graph: &GoalTaskGraph, task_id: &str) -> bool {
    task_graph
        .tasks
        .iter()
        .any(|task| task.id == task_id && task.status == GoalTaskStatus::Done)
}

pub(crate) fn goal_task_dependencies_done(task_graph: &GoalTaskGraph, task: &GoalTask) -> bool {
    task.dependencies
        .iter()
        .all(|dependency| goal_task_done(task_graph, dependency))
}

pub(crate) fn pending_goal_agent_followup_proposals(
    task_graph: &GoalTaskGraph,
) -> Vec<super::agent::GoalAgentTaskProposal> {
    task_graph
        .tasks
        .iter()
        .filter(|task| {
            task.status == GoalTaskStatus::Pending
                && task.owner_role.as_deref() == Some(super::state::GOAL_AGENT_WORKER_ROLE)
                && task.id != super::state::GOAL_AGENT_EXECUTE_TASK_ID
                && goal_task_dependencies_done(task_graph, task)
        })
        .map(goal_agent_proposal_from_task)
        .collect()
}

fn goal_agent_proposal_from_task(task: &GoalTask) -> super::agent::GoalAgentTaskProposal {
    super::agent::GoalAgentTaskProposal {
        id: task.id.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        dependencies: task.dependencies.clone(),
        read_set: task.read_set.clone(),
        write_set: task.write_set.clone(),
        risk: task.risk.clone(),
        acceptance: task.acceptance.clone(),
        budget_secs: super::state::default_goal_agent_task_budget_secs(),
        priority: 0,
    }
}

pub(crate) fn summarize_task_graph(task_graph: &GoalTaskGraph) -> GoalTaskGraphSummary {
    GoalTaskGraphSummary {
        total_tasks: task_graph.tasks.len(),
        pending_tasks: task_graph
            .tasks
            .iter()
            .filter(|task| task.status == GoalTaskStatus::Pending)
            .count(),
        blocked_tasks: task_graph
            .tasks
            .iter()
            .filter(|task| task.status == GoalTaskStatus::Blocked)
            .count(),
        done_tasks: task_graph
            .tasks
            .iter()
            .filter(|task| task.status == GoalTaskStatus::Done)
            .count(),
    }
}

pub(crate) fn apply_agent_execution_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalAgentRunEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == super::state::GOAL_AGENT_EXECUTE_TASK_ID)?;
    let success =
        evidence.summary.completed == evidence.summary.total && evidence.summary.failed == 0;

    task.status = if success {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_AGENT_WORKER_ROLE.to_string());
    task.completed_at = success.then_some(completed_at);
    record_goal_task_attempt_result(task, success);
    task.evidence = super::evidence::agent_execution_task_evidence(evidence, success);
    Some(task.clone())
}

pub(crate) fn apply_agent_followup_task_results(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalAgentRunEvidence,
    completed_at: DateTime<Utc>,
) {
    for task in task_graph.tasks.iter_mut().filter(|task| {
        evidence
            .accepted_task_ids
            .iter()
            .any(|task_id| task_id == &task.id)
    }) {
        let result = evidence
            .worker_results
            .iter()
            .find(|result| result.task_id == task.id);
        let success = result
            .map(|result| {
                matches!(
                    result.status,
                    crate::runtime::worker::ResultStatus::Success
                        | crate::runtime::worker::ResultStatus::Partial
                )
            })
            .unwrap_or(false);
        task.status = if success {
            GoalTaskStatus::Done
        } else {
            GoalTaskStatus::Blocked
        };
        task.completed_at = success.then_some(completed_at);
        record_goal_task_attempt_result(task, success);
        task.evidence = super::evidence::agent_followup_task_evidence(evidence, result, success);
    }
}

fn record_goal_task_attempt_result(task: &mut GoalTask, success: bool) {
    task.lease_expires_at = None;
    if !success {
        task.retry_count = task.retry_count.saturating_add(1);
    }
}

pub(crate) async fn apply_agent_proposed_task_mutations(
    state: &GoalState,
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalAgentRunEvidence,
    recorded_at: DateTime<Utc>,
) -> Result<()> {
    if evidence.agent_proposed_tasks.is_empty() {
        return Ok(());
    }

    let policy = super::agent::validate_goal_agent_task_proposals(
        state,
        task_graph,
        &evidence.summary.run_id,
        evidence.agent_proposed_tasks.clone(),
        false,
    );
    write_json_artifact(
        &state.state_dir.join(&evidence.agent_task_proposals_path),
        &policy,
    )
    .await?;
    append_agent_proposed_task_events(state, evidence, &policy).await?;

    let previous_task_count = task_graph.tasks.len();
    for proposal in &policy.accepted_tasks {
        task_graph.tasks.push(goal_task_from_agent_proposal(
            proposal,
            &evidence.agent_task_proposals_path,
            recorded_at,
        ));
    }
    append_task_graph_mutation_events(state, evidence, &policy, previous_task_count).await?;

    Ok(())
}

fn goal_task_from_agent_proposal(
    proposal: &super::agent::GoalAgentTaskProposal,
    proposal_path: &Path,
    recorded_at: DateTime<Utc>,
) -> GoalTask {
    GoalTask {
        id: proposal.id.clone(),
        title: proposal.title.clone(),
        description: proposal.description.clone(),
        status: GoalTaskStatus::Pending,
        owner_role: Some(GOAL_AGENT_WORKER_ROLE.to_string()),
        completed_at: None,
        evidence: vec![GoalTaskEvidence {
            kind: "agent_proposal".to_string(),
            path: proposal_path.to_path_buf(),
            summary: format!(
                "Accepted agent-proposed follow-up task at {recorded_at}: {}",
                proposal.title
            ),
        }],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: proposal.dependencies.clone(),
        read_set: proposal.read_set.clone(),
        write_set: proposal.write_set.clone(),
        risk: proposal.risk.clone(),
        acceptance: proposal.acceptance.clone(),
    }
}

async fn append_agent_proposed_task_events(
    state: &GoalState,
    evidence: &GoalAgentRunEvidence,
    policy: &GoalAgentTaskPolicy,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let run_id = &evidence.summary.run_id;

    for proposal in &policy.proposed_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskProposed,
        )
        .with_actor(super::state::GOAL_AGENT_WORKER_ID)
        .with_payload(super::agent::goal_agent_task_policy_payload(proposal, None))?;
        writer.append(&event).await?;
    }

    for proposal in &policy.accepted_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskAccepted,
        )
        .with_actor(super::state::GOAL_CONTROLLER_ACTOR)
        .with_payload(super::agent::goal_agent_task_policy_payload(
            proposal,
            Some("accepted agent-proposed task graph mutation"),
        ))?;
        writer.append(&event).await?;
    }

    for decision in &policy.rejected_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskRejected,
        )
        .with_actor(super::state::GOAL_CONTROLLER_ACTOR)
        .with_payload(super::agent::goal_agent_task_policy_payload(
            &decision.task,
            Some(&decision.reason),
        ))?;
        writer.append(&event).await?;
    }

    Ok(())
}

async fn append_task_graph_mutation_events(
    state: &GoalState,
    evidence: &GoalAgentRunEvidence,
    policy: &GoalAgentTaskPolicy,
    previous_task_count: usize,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let run_id = &evidence.summary.run_id;

    for (index, proposal) in policy.accepted_tasks.iter().enumerate() {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskGraphMutated,
        )
        .with_actor(super::state::GOAL_CONTROLLER_ACTOR)
        .with_payload(crate::runtime::events::TaskGraphMutationPayload {
            action: "task_added".to_string(),
            source: "agent_proposal".to_string(),
            task_id: crate::runtime::events::TaskId(proposal.id.clone()),
            task_graph_path: PathBuf::from(super::state::GOAL_TASK_GRAPH_FILE),
            proposal_path: evidence.agent_task_proposals_path.clone(),
            total_tasks_after: previous_task_count + index + 1,
        })?;
        writer.append(&event).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, dependencies: &[&str]) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status: GoalTaskStatus::Pending,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: dependencies
                .iter()
                .map(|dependency| dependency.to_string())
                .collect(),
            read_set: Vec::new(),
            write_set: Vec::new(),
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
        GoalTaskGraph {
            version: 1,
            goal_id: "goal-test".to_string(),
            generated_at: Utc::now(),
            tasks,
        }
    }

    #[test]
    fn validate_accepts_dependency_dag() {
        let graph = graph(vec![
            task("goal-intake", &[]),
            task("goal-plan", &["goal-intake"]),
            task("goal-verify", &["goal-plan"]),
        ]);

        graph.validate().expect("valid graph should pass");
    }

    #[test]
    fn validate_rejects_duplicate_task_ids() {
        let graph = graph(vec![task("goal-intake", &[]), task("goal-intake", &[])]);

        let err = graph.validate().expect_err("duplicate ids must fail");

        assert!(
            err.to_string().contains("duplicate task id: goal-intake"),
            "{err}"
        );
    }

    #[test]
    fn validate_rejects_unknown_dependencies() {
        let graph = graph(vec![task("goal-verify", &["goal-plan"])]);

        let err = graph
            .validate()
            .expect_err("unknown dependencies must fail");

        assert!(
            err.to_string()
                .contains("task goal-verify depends on missing task goal-plan"),
            "{err}"
        );
    }

    #[test]
    fn validate_rejects_dependency_cycles() {
        let graph = graph(vec![
            task("goal-a", &["goal-c"]),
            task("goal-b", &["goal-a"]),
            task("goal-c", &["goal-b"]),
        ]);

        let err = graph.validate().expect_err("dependency cycles must fail");

        assert!(
            err.to_string()
                .contains("task graph contains a dependency cycle"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn load_defaults_retry_and_lease_metadata_for_legacy_graph() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": Utc::now(),
            "tasks": [
                {
                    "id": "goal-intake",
                    "title": "Task goal-intake",
                    "description": "Task goal-intake description",
                    "status": "pending",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": [],
                    "risk": "low",
                    "acceptance": ["Task goal-intake acceptance"]
                }
            ]
        });
        tokio::fs::write(
            tmp.path().join(GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&graph_json).expect("json"),
        )
        .await
        .expect("write legacy graph");

        let graph = GoalTaskGraph::load(tmp.path())
            .await
            .expect("legacy graph should load");

        assert_eq!(graph.tasks[0].retry_count, 0);
        assert_eq!(graph.tasks[0].max_retries, 0);
        assert!(graph.tasks[0].lease_expires_at.is_none());
    }
}
