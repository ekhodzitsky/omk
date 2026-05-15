use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use super::{
    check_task_path_policy, evaluate_task_budget, extract_goal_agent_task_proposals,
    goal_agent_lease_seconds_override, goal_agent_scheduler_tasks, goal_agent_task_policy_payload,
    goal_agent_wire_runtime_available, goal_agent_worker_count, goal_agent_worker_name,
    prepare_goal_agent_workers, read_goal_agent_worker_results, stop_wire_worker,
    summarize_goal_agent_worker_results, task_dispatch_accepted_payload,
    task_dispatch_rejected_payload, validate_goal_agent_task_proposals,
    watch_goal_control_interrupt, write_goal_agent_mutation_snapshot, write_json_artifact, Event,
    EventBuilder, EventKind, EventWriter, GoalAgentDispatchPlan, GoalAgentRunEvidence, GoalState,
    GoalTaskGraph, RunId, TeamRunner, WireWorkerAdapter, WorkerId, EVENTS_FILE,
    GOAL_AGENT_RUNS_DIR, GOAL_AGENT_TASK_POLICY_FILE, GOAL_AGENT_TASK_PROPOSALS_FILE,
    GOAL_AGENT_WORKER_ROLE, GOAL_ARTIFACTS_DIR, GOAL_CONTROLLER_ACTOR, OUTBOX_FILE, WORKERS_DIR,
};

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
    let agent_task_proposals_path = run_path.join(GOAL_AGENT_TASK_PROPOSALS_FILE);
    let mutation_diff_path = run_path.join("mutation.diff");
    let changed_files_path = run_path.join("changed-files.json");

    let event_writer = EventWriter::new(state.state_dir.join(EVENTS_FILE));
    let builder = EventBuilder::new(RunId(run_id.clone()));
    let policy = validate_goal_agent_task_proposals(
        state,
        task_graph,
        &run_id,
        dispatch.proposals.clone(),
        dispatch.allow_existing_task_ids,
    );
    write_json_artifact(&state.state_dir.join(&task_policy_path), &policy).await?;

    let wave_rejected: std::collections::HashMap<String, String> = policy
        .rejected_tasks
        .iter()
        .map(|r| (r.task.id.clone(), r.reason.clone()))
        .collect();

    let mut dispatch_accepted = Vec::new();
    let mut dispatch_rejected_count = 0;

    for proposal in &policy.proposed_tasks {
        let proposed_event = Event::new(RunId(run_id.clone()), EventKind::TaskProposed)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(proposal, None))?;
        event_writer.append(&proposed_event).await?;

        if let Some(reason) = wave_rejected.get(&proposal.id) {
            let rejected_event = Event::new(RunId(run_id.clone()), EventKind::TaskRejected)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(task_dispatch_rejected_payload(proposal, reason, None)?)?;
            event_writer.append(&rejected_event).await?;
            dispatch_rejected_count += 1;
            continue;
        }

        match evaluate_task_budget(state, proposal).await {
            Ok(snapshot) => {
                if let Some(reason) = check_task_path_policy(proposal) {
                    let rejected_event = Event::new(RunId(run_id.clone()), EventKind::TaskRejected)
                        .with_actor(GOAL_CONTROLLER_ACTOR)
                        .with_payload(task_dispatch_rejected_payload(
                            proposal,
                            &reason,
                            Some(&snapshot),
                        )?)?;
                    event_writer.append(&rejected_event).await?;
                    dispatch_rejected_count += 1;
                } else {
                    let accepted_event = Event::new(RunId(run_id.clone()), EventKind::TaskAccepted)
                        .with_actor(GOAL_CONTROLLER_ACTOR)
                        .with_payload(task_dispatch_accepted_payload(proposal, &snapshot)?)?;
                    event_writer.append(&accepted_event).await?;
                    dispatch_accepted.push(proposal.clone());
                }
            }
            Err(reason) => {
                let rejected_event = Event::new(RunId(run_id.clone()), EventKind::TaskRejected)
                    .with_actor(GOAL_CONTROLLER_ACTOR)
                    .with_payload(task_dispatch_rejected_payload(proposal, &reason, None)?)?;
                event_writer.append(&rejected_event).await?;
                dispatch_rejected_count += 1;
            }
        }
    }

    let accepted_task_ids: Vec<String> = dispatch_accepted
        .iter()
        .map(|task| task.id.clone())
        .collect();
    let accepted_task_count = dispatch_accepted.len();
    let rejected_task_count = dispatch_rejected_count;
    let scheduler_tasks = goal_agent_scheduler_tasks(
        state,
        task_graph,
        started_at,
        &dispatch.run_key,
        &dispatch_accepted,
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
