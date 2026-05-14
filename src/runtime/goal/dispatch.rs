use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::agent::{
    append_goal_agent_task_policy_events, validate_goal_agent_task_proposals, GoalAgentDispatchPlan,
};
use super::evidence::{
    extract_goal_agent_task_proposals, write_goal_agent_mutation_snapshot, GoalAgentRunEvidence,
};
use super::proof::write_json_artifact;
use super::state::{
    GoalState, GoalStatus, GOAL_AGENT_RUNS_DIR, GOAL_AGENT_TASK_POLICY_FILE, GOAL_AGENT_WORKER_ID,
    GOAL_AGENT_WORKER_ROLE, GOAL_ARTIFACTS_DIR,
};
use super::task_graph::{GoalTaskGraph, GoalTaskStatus};
use crate::runtime::config::{HEARTBEAT_FILE, INBOX_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
use crate::runtime::scheduler::runner::TeamRunner;
use crate::runtime::scheduler::task::Task;
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::WorkerSpec;

pub(crate) fn goal_agent_wire_runtime_available() -> bool {
    std::env::var_os("MOCK_KIMI").is_some() || which::which("kimi").is_ok()
}

fn goal_interrupt_poll_interval() -> Duration {
    std::env::var("OMK_GOAL_INTERRUPT_POLL_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(500))
}

async fn watch_goal_control_interrupt(
    goal_dir: PathBuf,
    worker_cancel: CancellationToken,
    monitor_cancel: CancellationToken,
) -> Option<GoalStatus> {
    loop {
        tokio::select! {
            biased;
            _ = monitor_cancel.cancelled() => return None,
            _ = tokio::time::sleep(goal_interrupt_poll_interval()) => {
                let Ok(state) = GoalState::load(&goal_dir).await else {
                    continue;
                };
                if matches!(state.status, GoalStatus::Paused | GoalStatus::Cancelled) {
                    worker_cancel.cancel();
                    return Some(state.status);
                }
            }
        }
    }
}

pub(crate) async fn stop_wire_worker(handle: &mut JoinHandle<()>) {
    if tokio::time::timeout(Duration::from_secs(2), &mut *handle)
        .await
        .is_err()
    {
        handle.abort();
        let _ = handle.await;
    }
}

pub(crate) async fn run_goal_agent_task_wave(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    project_dir: &Path,
    started_at: DateTime<Utc>,
    dispatch: &GoalAgentDispatchPlan,
) -> Result<GoalAgentRunEvidence> {
    let run_id = format!("{}-{}", state.goal_id, dispatch.run_key);
    let run_path = PathBuf::from(GOAL_ARTIFACTS_DIR)
        .join(GOAL_AGENT_RUNS_DIR)
        .join(&dispatch.run_key);
    let run_dir = state.state_dir.join(&run_path);
    crate::runtime::config::ensure_private_dir(&run_dir).await?;

    let primary_worker_name = goal_agent_worker_name(0);
    let worker_outbox_path = run_path
        .join(WORKERS_DIR)
        .join(&primary_worker_name)
        .join(OUTBOX_FILE);
    let wire_events_path = run_path
        .join(WORKERS_DIR)
        .join(&primary_worker_name)
        .join("wire-events.jsonl");
    let task_policy_path = run_path.join(GOAL_AGENT_TASK_POLICY_FILE);
    let agent_task_proposals_path = run_path.join(super::state::GOAL_AGENT_TASK_PROPOSALS_FILE);
    let mutation_diff_path = run_path.join("mutation.diff");
    let changed_files_path = run_path.join("changed-files.json");

    let event_writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(run_id.clone()));
    let policy = validate_goal_agent_task_proposals(
        state,
        task_graph,
        &run_id,
        dispatch.proposals.clone(),
        dispatch.allow_existing_task_ids,
    );
    write_json_artifact(&state.state_dir.join(&task_policy_path), &policy).await?;
    append_goal_agent_task_policy_events(&event_writer, &run_id, &policy).await?;

    let accepted_task_ids: Vec<String> = policy
        .accepted_tasks
        .iter()
        .map(|task| task.id.clone())
        .collect();
    let accepted_task_count = policy.accepted_tasks.len();
    let rejected_task_count = policy.rejected_tasks.len();
    let scheduler_tasks = goal_agent_scheduler_tasks(
        state,
        task_graph,
        started_at,
        &dispatch.run_key,
        &policy.accepted_tasks,
    );
    let run_description = format!(
        "goal controller agent wave {}: accepted={}, rejected={}, max_agents={}",
        dispatch.run_key, accepted_task_count, rejected_task_count, policy.max_agents
    );

    event_writer
        .append(&builder.run_started("goal-agent", project_dir, &run_description)?)
        .await?;

    if scheduler_tasks.is_empty() {
        let summary =
            "Goal controller rejected all proposed agent tasks; no safe work is dispatchable";
        event_writer.append(&builder.run_failed(summary)?).await?;
        let changed_files = write_goal_agent_mutation_snapshot(
            state,
            project_dir,
            &mutation_diff_path,
            &changed_files_path,
        )
        .await?;
        return Ok(GoalAgentRunEvidence {
            summary: crate::runtime::scheduler::runner::RunSummary {
                run_id,
                completed: 0,
                failed: 1,
                cancelled: 0,
                total: 1,
            },
            run_path,
            task_policy_path,
            agent_task_proposals_path,
            worker_outbox_path,
            wire_events_path,
            mutation_diff_path,
            changed_files_path,
            changed_files,
            accepted_task_count,
            rejected_task_count,
            accepted_task_ids,
            agent_proposed_tasks: Vec::new(),
            worker_results: Vec::new(),
            worker_summary: Some(summary.to_string()),
        });
    }

    if !goal_agent_wire_runtime_available() {
        let summary = "Kimi CLI not found; install/authenticate kimi or set MOCK_KIMI to a mock binary before running goal agent execution";
        event_writer.append(&builder.run_failed(summary)?).await?;
        let changed_files = write_goal_agent_mutation_snapshot(
            state,
            project_dir,
            &mutation_diff_path,
            &changed_files_path,
        )
        .await?;
        return Ok(GoalAgentRunEvidence {
            summary: crate::runtime::scheduler::runner::RunSummary {
                run_id,
                completed: 0,
                failed: accepted_task_count,
                cancelled: 0,
                total: accepted_task_count,
            },
            run_path,
            task_policy_path,
            agent_task_proposals_path,
            worker_outbox_path,
            wire_events_path,
            mutation_diff_path,
            changed_files_path,
            changed_files,
            accepted_task_count,
            rejected_task_count,
            accepted_task_ids,
            agent_proposed_tasks: Vec::new(),
            worker_results: Vec::new(),
            worker_summary: Some(summary.to_string()),
        });
    }

    let worker_count = goal_agent_worker_count(policy.max_agents, scheduler_tasks.len());
    let worker_specs = prepare_goal_agent_workers(&run_dir, project_dir, worker_count).await?;
    for spec in &worker_specs {
        event_writer
            .append(&builder.worker_started(WorkerId(spec.name.clone()), GOAL_AGENT_WORKER_ROLE)?)
            .await?;
    }

    let cancel = CancellationToken::new();
    let mut handles = Vec::with_capacity(worker_specs.len());
    for spec in &worker_specs {
        let adapter = WireWorkerAdapter::new_with_cancel(
            spec.clone(),
            RunId(run_id.clone()),
            event_writer.clone(),
            cancel.clone(),
        );
        handles.push(adapter.spawn());
    }
    let mut runner = TeamRunner::init_with_tasks(
        &run_id,
        project_dir,
        &run_dir,
        event_writer,
        scheduler_tasks,
    )
    .await?;
    if let Some(lease_secs) = goal_agent_lease_seconds_override() {
        runner.set_lease_seconds(lease_secs);
    }

    let monitor_cancel = CancellationToken::new();
    let monitor_handle = tokio::spawn(watch_goal_control_interrupt(
        state.state_dir.clone(),
        cancel.clone(),
        monitor_cancel.clone(),
    ));

    let run_result = runner
        .run_with_cancel_reason(&worker_specs, &cancel, "cancelled by user")
        .await;
    monitor_cancel.cancel();
    let _ = monitor_handle.await;
    cancel.cancel();
    for handle in &mut handles {
        stop_wire_worker(handle).await;
    }

    let summary = run_result?;
    let worker_results = read_goal_agent_worker_results(&worker_specs, &accepted_task_ids).await?;
    let worker_summary = summarize_goal_agent_worker_results(&worker_results)
        .or_else(|| (summary.cancelled > 0).then(|| "cancelled by user".to_string()));
    let agent_proposed_tasks = extract_goal_agent_task_proposals(&worker_results);
    let changed_files = write_goal_agent_mutation_snapshot(
        state,
        project_dir,
        &mutation_diff_path,
        &changed_files_path,
    )
    .await?;

    Ok(GoalAgentRunEvidence {
        summary,
        run_path,
        task_policy_path,
        agent_task_proposals_path,
        worker_outbox_path,
        wire_events_path,
        mutation_diff_path,
        changed_files_path,
        changed_files,
        accepted_task_count,
        rejected_task_count,
        accepted_task_ids,
        agent_proposed_tasks,
        worker_results,
        worker_summary,
    })
}

fn goal_agent_worker_name(index: usize) -> String {
    if index == 0 {
        GOAL_AGENT_WORKER_ID.to_string()
    } else {
        format!("goal-agent-worker-{index}")
    }
}

fn goal_agent_worker_count(max_agents: usize, task_count: usize) -> usize {
    max_agents.max(1).min(task_count.max(1))
}

fn goal_agent_lease_seconds_override() -> Option<i64> {
    std::env::var("OMK_GOAL_AGENT_LEASE_SECS")
        .ok()
        .and_then(|raw| raw.trim().parse::<i64>().ok())
        .filter(|secs| *secs > 0)
}

async fn prepare_goal_agent_workers(
    run_dir: &Path,
    project_dir: &Path,
    worker_count: usize,
) -> Result<Vec<WorkerSpec>> {
    let mut specs = Vec::with_capacity(worker_count);
    for index in 0..worker_count {
        let name = goal_agent_worker_name(index);
        let worker_dir = run_dir.join(WORKERS_DIR).join(&name);
        crate::runtime::config::ensure_private_dir(&worker_dir).await?;
        let spec = WorkerSpec {
            name,
            role: GOAL_AGENT_WORKER_ROLE.to_string(),
            inbox: worker_dir.join(INBOX_FILE),
            outbox: worker_dir.join(OUTBOX_FILE),
            heartbeat: worker_dir.join(HEARTBEAT_FILE),
            project_dir: Some(project_dir.to_path_buf()),
        };
        spec.save().await?;
        tokio::fs::write(&spec.inbox, b"").await?;
        tokio::fs::write(&spec.outbox, b"").await?;
        tokio::fs::write(worker_dir.join("wire-events.jsonl"), b"").await?;
        specs.push(spec);
    }
    Ok(specs)
}

fn goal_agent_scheduler_tasks(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
    controller_task_id: &str,
    proposals: &[super::agent::GoalAgentTaskProposal],
) -> Vec<Task> {
    proposals
        .iter()
        .map(|proposal| {
            let mut task = Task::new(proposal.id.clone(), proposal.title.clone())
                .with_description(goal_agent_task_prompt(
                    state,
                    task_graph,
                    generated_at,
                    proposal,
                ))
                .with_dependencies(proposal.dependencies.clone())
                .with_read_set(proposal.read_set.clone())
                .with_write_set(proposal.write_set.clone())
                .with_priority(proposal.priority)
                .with_max_retries(0);
            task.extra.insert(
                "acceptance".to_string(),
                serde_json::json!(proposal.acceptance),
            );
            task.extra.insert(
                "budget_secs".to_string(),
                serde_json::json!(proposal.budget_secs),
            );
            task.extra
                .insert("risk".to_string(), serde_json::json!(proposal.risk));
            task.extra.insert(
                "controller_task_id".to_string(),
                serde_json::json!(controller_task_id),
            );
            task
        })
        .collect()
}

fn goal_agent_task_prompt(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
    proposal: &super::agent::GoalAgentTaskProposal,
) -> String {
    let local_status = task_graph
        .tasks
        .iter()
        .find(|task| task.id == super::state::GOAL_LOCAL_VERIFY_TASK_ID)
        .map(|task| task.status.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "Goal ID: {}\nGenerated: {generated_at}\n\nOriginal goal:\n{}\n\nNormalized goal:\n{}\n\nController task: {}\nTitle: {}\nBudget: {} seconds\nRisk: {}\n\nTask:\n{}\n\nAcceptance criteria:\n- {}\n\nPolicy:\nStay inside the current repository, keep the diff minimal, do not commit, do not publish, do not touch secrets, and summarize changed files plus verification still needed for production readiness.\n\nLocal verification task status: {local_status}",
        state.goal_id,
        state.original_goal,
        state.normalized_goal,
        proposal.id,
        proposal.title,
        proposal.budget_secs,
        proposal.risk,
        proposal.description,
        proposal.acceptance.join("\n- ")
    )
}

async fn read_goal_agent_worker_results(
    specs: &[WorkerSpec],
    task_ids: &[String],
) -> Result<Vec<crate::runtime::worker::WorkerResult>> {
    let mut filtered = Vec::new();
    for spec in specs {
        let results: Vec<crate::runtime::worker::WorkerResult> = spec.read_results().await?;
        filtered.extend(
            results
                .into_iter()
                .filter(|result| task_ids.iter().any(|task_id| task_id == &result.task_id)),
        );
    }
    Ok(filtered)
}

fn summarize_goal_agent_worker_results(
    results: &[crate::runtime::worker::WorkerResult],
) -> Option<String> {
    let summaries: Vec<String> = results
        .iter()
        .map(|result| format!("{}: {}", result.task_id, result.summary))
        .collect();
    (!summaries.is_empty()).then(|| summaries.join(" | "))
}

pub(crate) async fn append_agent_execution_task_events(
    state: &GoalState,
    task: &super::task_graph::GoalTask,
    evidence: &GoalAgentRunEvidence,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let run_id = RunId(state.goal_id.clone());
    let task_id = TaskId(task.id.clone());
    let worker_id = WorkerId(GOAL_AGENT_WORKER_ID.to_string());
    let summary = format!(
        "{} via {} (run: {}, scheduler: {})",
        super::planner::controller_task_summary(task),
        GOAL_AGENT_WORKER_ID,
        evidence.run_path.display(),
        evidence.summary.run_id
    );
    let event = if task.status == GoalTaskStatus::Done {
        EventBuilder::new(run_id).task_completed(task_id, worker_id, Some(&summary))?
    } else {
        Event::new(run_id, EventKind::TaskFailed)
            .with_actor(GOAL_AGENT_WORKER_ID)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": GOAL_AGENT_WORKER_ID,
                "summary": summary,
            }))?
    };
    writer.append(&event).await
}
