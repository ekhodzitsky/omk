use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::agent::GoalAgentTaskPolicy;
use super::evidence::GoalAgentRunEvidence;
use super::proof::write_json_artifact;
use super::state::{GoalState, GOAL_AGENT_WORKER_ROLE, GOAL_TASK_GRAPH_FILE};

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
        let graph = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
        Ok(graph)
    }
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
        task.evidence = super::evidence::agent_followup_task_evidence(evidence, result, success);
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

    for proposal in &policy.accepted_tasks {
        task_graph.tasks.push(goal_task_from_agent_proposal(
            proposal,
            &evidence.agent_task_proposals_path,
            recorded_at,
        ));
    }

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
