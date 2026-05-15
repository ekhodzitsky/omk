use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::model::{GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::goal::agent::{GoalAgentTaskPolicy, GoalAgentTaskProposal};
use crate::runtime::goal::evidence::GoalAgentRunEvidence;
use crate::runtime::goal::proof::write_json_artifact;
use crate::runtime::goal::state::{
    default_goal_agent_task_budget_secs, GoalState, GOAL_AGENT_EXECUTE_TASK_ID,
    GOAL_AGENT_WORKER_ID, GOAL_AGENT_WORKER_ROLE, GOAL_CONTROLLER_ACTOR, GOAL_TASK_GRAPH_FILE,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

fn goal_task_dependencies_done(task_graph: &GoalTaskGraph, task: &GoalTask) -> bool {
    task.dependencies
        .iter()
        .all(|dependency| goal_task_done(task_graph, dependency))
}

pub(crate) fn pending_goal_agent_followup_proposals(
    task_graph: &GoalTaskGraph,
) -> Vec<GoalAgentTaskProposal> {
    task_graph
        .tasks
        .iter()
        .filter(|task| {
            task.status == GoalTaskStatus::Pending
                && task.owner_role.as_deref() == Some(GOAL_AGENT_WORKER_ROLE)
                && task.id != GOAL_AGENT_EXECUTE_TASK_ID
                && goal_task_dependencies_done(task_graph, task)
        })
        .map(goal_agent_proposal_from_task)
        .collect()
}

fn goal_agent_proposal_from_task(task: &GoalTask) -> GoalAgentTaskProposal {
    GoalAgentTaskProposal {
        id: task.id.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        dependencies: task.dependencies.clone(),
        read_set: task.read_set.clone(),
        write_set: task.write_set.clone(),
        risk: task.risk.clone(),
        acceptance: task.acceptance.clone(),
        budget_secs: default_goal_agent_task_budget_secs(),
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
        .find(|task| task.id == GOAL_AGENT_EXECUTE_TASK_ID)?;
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
    task.evidence =
        crate::runtime::goal::evidence::agent_execution_task_evidence(evidence, success);
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
        task.evidence =
            crate::runtime::goal::evidence::agent_followup_task_evidence(evidence, result, success);
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

    let policy = crate::runtime::goal::agent::validate_goal_agent_task_proposals(
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
    proposal: &GoalAgentTaskProposal,
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
        .with_actor(GOAL_AGENT_WORKER_ID)
        .with_payload(crate::runtime::goal::agent::goal_agent_task_policy_payload(
            proposal, None,
        ))?;
        writer.append(&event).await?;
    }

    for proposal in &policy.accepted_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskAccepted,
        )
        .with_actor(GOAL_CONTROLLER_ACTOR)
        .with_payload(crate::runtime::goal::agent::goal_agent_task_policy_payload(
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
        .with_actor(GOAL_CONTROLLER_ACTOR)
        .with_payload(crate::runtime::goal::agent::goal_agent_task_policy_payload(
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
        .with_actor(GOAL_CONTROLLER_ACTOR)
        .with_payload(crate::runtime::events::TaskGraphMutationPayload {
            action: "task_added".to_string(),
            source: "agent_proposal".to_string(),
            task_id: crate::runtime::events::TaskId(proposal.id.clone()),
            task_graph_path: PathBuf::from(GOAL_TASK_GRAPH_FILE),
            proposal_path: evidence.agent_task_proposals_path.clone(),
            total_tasks_after: previous_task_count + index + 1,
        })?;
        writer.append(&event).await?;
    }

    Ok(())
}
