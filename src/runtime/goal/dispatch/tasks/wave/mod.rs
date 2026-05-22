use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::runtime::config::{ensure_private_dir, EVENTS_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::db::DbHandle;
use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId};
use crate::runtime::goal::agent::leases::{LeaseError, LeaseManager};
use crate::runtime::goal::agent::GoalAgentDispatchPlan;
use crate::runtime::goal::dispatch::runtime::{
    goal_agent_wire_runtime_available, goal_agent_worker_name,
};
use crate::runtime::goal::dispatch::tasks::scheduler::goal_agent_scheduler_tasks;
use crate::runtime::goal::evidence::{write_goal_agent_mutation_snapshot, GoalAgentRunEvidence};
use crate::runtime::goal::state::{
    goals_db_path, GoalState, GOAL_AGENT_RUNS_DIR, GOAL_AGENT_TASK_POLICY_FILE,
    GOAL_AGENT_TASK_PROPOSALS_FILE, GOAL_AGENT_WORKER_ROLE, GOAL_ARTIFACTS_DIR,
};
use crate::runtime::goal::task_graph::{
    update_goal_task_delivery_metadata, GoalTaskDeliveryMetadataUpdate, GoalTaskGraph,
};
use crate::runtime::scheduler::runner::RunSummary;

mod policy;
mod results;
mod runner;

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
    ensure_private_dir(&run_dir).await?;

    let primary = goal_agent_worker_name(0);
    let worker_outbox_path = run_path.join(WORKERS_DIR).join(&primary).join(OUTBOX_FILE);
    let wire_events_path = run_path
        .join(WORKERS_DIR)
        .join(&primary)
        .join("wire-events.jsonl");
    let task_policy_path = run_path.join(GOAL_AGENT_TASK_POLICY_FILE);
    let agent_task_proposals_path = run_path.join(GOAL_AGENT_TASK_PROPOSALS_FILE);
    let mutation_diff_path = run_path.join("mutation.diff");
    let changed_files_path = run_path.join("changed-files.json");

    let event_writer = EventWriter::new(state.state_dir.join(EVENTS_FILE));
    let builder = EventBuilder::new(RunId(run_id.clone()));

    let (lease_guard, skipped) = match try_claim_slice_lease(
        state,
        dispatch,
        &event_writer,
        &builder,
        &run_id,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!(error = %e, "Slice lease claim failed; continuing without enforcement");
            (None, false)
        }
    };

    // If lease was skipped due to conflict/overlap, return a skipped evidence
    // so that execute_goal does NOT call cancel.cancel() (Option A).
    if skipped {
        let s = "Slice lease skipped due to conflict or write-set overlap";
        let changed_files = write_goal_agent_mutation_snapshot(
            state,
            project_dir,
            &mutation_diff_path,
            &changed_files_path,
        )
        .await?;
        return Ok(GoalAgentRunEvidence {
            summary: RunSummary {
                run_id,
                completed: 0,
                failed: 0,
                cancelled: 0,
                total: 0,
            },
            run_path,
            task_policy_path,
            agent_task_proposals_path,
            worker_outbox_path,
            wire_events_path,
            mutation_diff_path,
            changed_files_path,
            changed_files,
            accepted_task_count: 0,
            rejected_task_count: 0,
            accepted_task_ids: Vec::new(),
            agent_proposed_tasks: Vec::new(),
            worker_results: Vec::new(),
            worker_summary: Some(s.to_string()),
        });
    }

    let run_id_inner = run_id.clone();
    let event_writer_inner = event_writer.clone();

    let result = async {
        let (dispatch_accepted, dispatch_rejected_count, policy) = policy::validate_and_classify_tasks(
            state,
            task_graph,
            &run_id_inner,
            dispatch,
            &event_writer_inner,
            &task_policy_path,
        )
        .await?;

        let accepted_task_ids: Vec<String> = dispatch_accepted.iter().map(|t| t.id.clone()).collect();
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

        event_writer_inner
            .append(&builder.run_started("goal-agent", project_dir, &run_description)?)
            .await?;

        if scheduler_tasks.is_empty() {
            let s = "Goal controller rejected all proposed agent tasks; no safe work is dispatchable";
            event_writer_inner.append(&builder.run_failed(s)?).await?;
            let changed_files = write_goal_agent_mutation_snapshot(
                state,
                project_dir,
                &mutation_diff_path,
                &changed_files_path,
            )
            .await?;
            return Ok(GoalAgentRunEvidence {
                summary: RunSummary {
                    run_id: run_id_inner,
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
                worker_summary: Some(s.to_string()),
            });
        }

        if !goal_agent_wire_runtime_available() {
            let s = "Kimi CLI not found; install/authenticate kimi or set MOCK_KIMI to a mock binary before running goal agent execution";
            event_writer_inner.append(&builder.run_failed(s)?).await?;
            let changed_files = write_goal_agent_mutation_snapshot(
                state,
                project_dir,
                &mutation_diff_path,
                &changed_files_path,
            )
            .await?;
            return Ok(GoalAgentRunEvidence {
                summary: RunSummary {
                    run_id: run_id_inner,
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
                worker_summary: Some(s.to_string()),
            });
        }

        let (summary, worker_specs) = runner::execute_wave_run(
            &run_id_inner,
            project_dir,
            &run_dir,
            &state.state_dir,
            event_writer_inner,
            &builder,
            scheduler_tasks,
            policy.max_agents,
        )
        .await?;

        let (worker_results, worker_summary, agent_proposed_tasks, changed_files) =
            results::gather_wave_results(
                &worker_specs,
                &accepted_task_ids,
                state,
                project_dir,
                &mutation_diff_path,
                &changed_files_path,
                &summary,
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
    .await;

    if let Some(guard) = lease_guard {
        let event = Event::new(RunId(run_id.clone()), EventKind::SliceLeaseReleased)
            .with_actor("lease-manager")
            .with_payload(serde_json::json!({
                "goal_id": state.goal_id,
                "slice_id": dispatch.proposals.first().map(|p| p.id.clone()).unwrap_or_default(),
                "lease_id": guard.lease_id(),
                "pid": std::process::id(),
                "role": GOAL_AGENT_WORKER_ROLE,
            }))?;
        let _ = event_writer.append(&event).await;
        guard.release().await;
    }

    result
}

async fn try_claim_slice_lease(
    state: &GoalState,
    dispatch: &GoalAgentDispatchPlan,
    event_writer: &EventWriter,
    _builder: &EventBuilder,
    run_id: &str,
) -> Result<
    (
        Option<crate::runtime::goal::agent::leases::LeaseGuard>,
        bool,
    ),
    LeaseError,
> {
    if dispatch.proposals.len() != 1 {
        return Ok((None, false));
    }

    let proposal = &dispatch.proposals[0];
    let slice_id = &proposal.id;
    let write_set = proposal.write_set.clone();

    let db = match DbHandle::open(goals_db_path()).await {
        Ok(db) => db,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to open goal DB for slice lease; continuing without enforcement");
            return Ok((None, false));
        }
    };

    let manager = Arc::new(LeaseManager::new(db));

    match manager
        .try_claim(&state.goal_id, slice_id, GOAL_AGENT_WORKER_ROLE, write_set)
        .await
    {
        Ok((guard, expired_leases)) => {
            // Emit expired events for any leases that were cleaned up
            for expired in &expired_leases {
                let event = Event::new(RunId(run_id.to_string()), EventKind::SliceLeaseExpired)
                    .with_actor("lease-manager")
                    .with_payload(serde_json::json!({
                        "goal_id": expired.goal_id,
                        "slice_id": expired.slice_id,
                        "lease_id": expired.lease_id,
                        "expired_at_unix": chrono::Utc::now().timestamp(),
                    }))
                    .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;
                let _ = event_writer.append(&event).await;
            }

            let event = Event::new(RunId(run_id.to_string()), EventKind::SliceLeaseClaimed)
                .with_actor("lease-manager")
                .with_payload(serde_json::json!({
                    "goal_id": state.goal_id,
                    "slice_id": slice_id,
                    "lease_id": guard.lease_id(),
                    "pid": manager.owner_pid,
                    "role": GOAL_AGENT_WORKER_ROLE,
                    "write_set": proposal.write_set,
                }))
                .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;
            event_writer
                .append(&event)
                .await
                .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;

            // Best-effort metadata update.
            let _ = update_goal_task_delivery_metadata(
                &state.state_dir,
                slice_id,
                GoalTaskDeliveryMetadataUpdate {
                    slice_lease_id: Some(guard.lease_id().to_string()),
                    ..Default::default()
                },
            )
            .await;

            Ok((Some(guard), false))
        }
        Err(LeaseError::Conflict {
            lease_id,
            pid,
            role,
            slice_id,
        }) => {
            let event = Event::new(RunId(run_id.to_string()), EventKind::SliceLeaseSkipped)
                .with_actor("lease-manager")
                .with_payload(serde_json::json!({
                    "goal_id": state.goal_id,
                    "slice_id": slice_id,
                    "reason": "conflict",
                    "conflict_lease_id": lease_id,
                    "conflict_pid": pid,
                    "conflict_role": role,
                }))
                .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;
            event_writer
                .append(&event)
                .await
                .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;
            Ok((None, true))
        }
        Err(LeaseError::WriteSetOverlap { lease_id, overlap }) => {
            let event = Event::new(RunId(run_id.to_string()), EventKind::SliceLeaseSkipped)
                .with_actor("lease-manager")
                .with_payload(serde_json::json!({
                    "goal_id": state.goal_id,
                    "slice_id": slice_id,
                    "reason": "write_set_overlap",
                    "conflict_lease_id": lease_id,
                    "overlap": overlap,
                }))
                .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;
            event_writer
                .append(&event)
                .await
                .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;
            Ok((None, true))
        }
        Err(LeaseError::Db(e)) => {
            tracing::warn!(error = %e, "Lease DB error; continuing without enforcement");
            Ok((None, false))
        }
    }
}
