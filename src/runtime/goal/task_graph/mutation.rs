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

/// Returns true when the agent execution phase is considered complete.
/// In non-slice mode this checks the single `goal-agent-execute` task.
/// In slice mode it checks that all `goal-agent-implement-{i}` tasks are done.
pub(crate) fn goal_agent_execution_done(task_graph: &GoalTaskGraph) -> bool {
    if task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_AGENT_EXECUTE_TASK_ID)
    {
        return goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID);
    }
    let implement_tasks: Vec<_> = task_graph
        .tasks
        .iter()
        .filter(|task| task.id.starts_with("goal-agent-implement-"))
        .collect();
    if implement_tasks.is_empty() {
        return false;
    }
    implement_tasks
        .iter()
        .all(|task| task.status == GoalTaskStatus::Done)
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
    apply_agent_task_result_by_id(
        task_graph,
        GOAL_AGENT_EXECUTE_TASK_ID,
        evidence,
        completed_at,
    )
}

pub(crate) fn apply_agent_task_result_by_id(
    task_graph: &mut GoalTaskGraph,
    task_id: &str,
    evidence: &GoalAgentRunEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == task_id)?;
    let success =
        evidence.summary.completed == evidence.summary.total && evidence.summary.failed == 0;
    eprintln!("DEBUG apply_agent_task_result_by_id task_id={task_id} completed={} total={} failed={} success={}", evidence.summary.completed, evidence.summary.total, evidence.summary.failed, success);

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

/// Append a cleanup task to the task graph and wire the original slice task
/// to depend on it. Returns the cleanup task id when a new task was created,
/// or `None` if a cleanup task for this slice already exists.
pub(crate) fn spawn_cleanup_task(
    task_graph: &mut GoalTaskGraph,
    slice_task_id: &str,
    feedback: &str,
    changed_files: &[String],
    generated_at: DateTime<Utc>,
) -> Option<String> {
    let cleanup_task_id = format!("goal-agent-cleanup-{slice_task_id}");
    if task_graph
        .tasks
        .iter()
        .any(|task| task.id == cleanup_task_id)
    {
        return None;
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

#[cfg(test)]
mod merge_tests {
    use super::*;
    use crate::runtime::goal::evidence::GoalAgentRunEvidence;
    use crate::runtime::scheduler::runner::RunSummary;

    fn task(id: &str, status: GoalTaskStatus) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status,
            owner_role: None,
            completed_at: None,
            evidence: vec![],
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: vec![],
            read_set: vec![],
            write_set: vec!["src".to_string()],
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

    fn evidence(task_id: &str, completed: usize, failed: usize) -> GoalAgentRunEvidence {
        GoalAgentRunEvidence {
            summary: RunSummary {
                run_id: format!("run-{task_id}"),
                completed,
                failed,
                cancelled: 0,
                total: completed + failed,
            },
            run_path: PathBuf::new(),
            task_policy_path: PathBuf::new(),
            agent_task_proposals_path: PathBuf::new(),
            worker_outbox_path: PathBuf::new(),
            wire_events_path: PathBuf::new(),
            mutation_diff_path: PathBuf::new(),
            changed_files_path: PathBuf::new(),
            changed_files: vec![],
            accepted_task_count: 0,
            rejected_task_count: 0,
            accepted_task_ids: vec![task_id.to_string()],
            agent_proposed_tasks: vec![],
            worker_results: vec![],
            worker_summary: None,
        }
    }

    #[test]
    fn merge_updates_status_to_done() {
        let mut main = graph(vec![
            task("t1", GoalTaskStatus::Pending),
            task("t2", GoalTaskStatus::Pending),
        ]);
        let mut delta1 = graph(vec![task("t1", GoalTaskStatus::Done)]);
        delta1.tasks[0].completed_at = Some(Utc::now());
        let mut delta2 = graph(vec![task("t2", GoalTaskStatus::Done)]);
        delta2.tasks[0].completed_at = Some(Utc::now());

        merge_concurrent_slice_task_graphs(&mut main, &[delta1, delta2]);

        assert_eq!(main.tasks[0].status, GoalTaskStatus::Done);
        assert_eq!(main.tasks[1].status, GoalTaskStatus::Done);
    }

    #[test]
    fn merge_prefers_blocked_over_pending() {
        let mut main = graph(vec![task("t1", GoalTaskStatus::Pending)]);
        let delta = graph(vec![task("t1", GoalTaskStatus::Blocked)]);

        merge_concurrent_slice_task_graphs(&mut main, &[delta]);

        assert_eq!(main.tasks[0].status, GoalTaskStatus::Blocked);
    }

    #[test]
    fn apply_agent_task_result_by_id_sets_done() {
        let mut tg = graph(vec![task("t1", GoalTaskStatus::Pending)]);
        let ev = evidence("t1", 1, 0);
        let result = apply_agent_task_result_by_id(&mut tg, "t1", &ev, Utc::now());
        assert!(result.is_some());
        assert_eq!(tg.tasks[0].status, GoalTaskStatus::Done);
    }
}
