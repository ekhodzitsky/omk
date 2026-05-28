use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{
    FileSystemGoalStateStore, GoalPhase, GoalStateStore, GoalStatus,
};
use crate::runtime::goal::task_graph::{
    ready_delivery_slices, GoalDeliverySlice, GoalTaskGraph, GoalTaskStatus,
};
use crate::runtime::goal::{agent, budget, dispatch, evidence, state, supervisor, task_graph};

pub async fn execute_goal(goal_id: &str, project_dir: &Path) -> anyhow::Result<GoalProof> {
    let dispatcher = dispatch::DefaultGoalDispatcher;
    execute_goal_with_dispatcher(goal_id, project_dir, &dispatcher).await
}

pub async fn execute_goal_with_dispatcher<D: dispatch::GoalDispatcher + Clone + 'static>(
    goal_id: &str,
    project_dir: &Path,
    dispatcher: &D,
) -> anyhow::Result<GoalProof> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    super::ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal execute").await?;
    let phase_start = tokio::time::Instant::now();
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::Execution;
    state.updated_at = chrono::Utc::now();
    state.completed_at = None;
    FileSystemGoalStateStore::new().save(&state).await?;

    let _heartbeat = supervisor::claim_goal(&state.goal_id).await?;

    let now = chrono::Utc::now();
    let max_agents = state.max_agents.unwrap_or(1);
    let use_concurrent_slices = state.slice_execution && max_agents > 1;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let ready_slices = if use_concurrent_slices {
        ready_delivery_slices(&state.state_dir, &task_graph).await?
    } else {
        Vec::new()
    };
    let ready_slice_count = ready_slices.len();

    let verification_proof = if use_concurrent_slices && !ready_slices.is_empty() {
        super::verify_goal_with_slices(goal_id, project_dir, Some(&ready_slices)).await?
    } else {
        super::verify_goal_with_slices(goal_id, project_dir, None).await?
    };

    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let local_verify_done = task_graph.tasks.iter().any(|task| {
        task.id == state::GOAL_LOCAL_VERIFY_TASK_ID && task.status == GoalTaskStatus::Done
    });

    if !local_verify_done {
        return Ok(verification_proof);
    }

    let Some(dispatch) = agent::goal_agent_dispatch_plan(&state, &task_graph) else {
        return Ok(verification_proof);
    };

    let actual_worker_count = if use_concurrent_slices && ready_slice_count > 1 {
        max_agents.min(ready_slice_count) as u64
    } else {
        1
    };

    let (agent_evidence, proof_gates, proof_git, proof_changed_files, post_mutation_gates_ran) =
        if use_concurrent_slices && ready_slice_count > 1 {
            // Concurrent path: run up to max_agents slices in parallel
            let concurrency_limit = max_agents;
            let slices_to_run: Vec<_> = ready_slices.into_iter().take(concurrency_limit).collect();

            // 1. Spawn waves via JoinSet with CancellationToken
            let cancel = CancellationToken::new();
            let mut wave_set = tokio::task::JoinSet::new();
            for slice in slices_to_run {
                let state = state.clone();
                let task_graph = task_graph.clone();
                let dispatcher = dispatcher.clone();
                wave_set.spawn(async move {
                    let dispatch =
                        agent::goal_agent_slice_dispatch_plan(&state, &task_graph, &slice.task_id)
                            .ok_or_else(|| {
                                anyhow::anyhow!("no dispatch plan for slice {}", slice.task_id)
                            })?;
                    let evidence = dispatcher
                        .execute_wave(&state, &task_graph, &slice.worktree_path, now, &dispatch)
                        .await?;
                    Ok::<_, anyhow::Error>((slice, dispatch, evidence))
                });
            }

            let mut slice_results = Vec::with_capacity(wave_set.len());
            while let Some(res) = wave_set.join_next().await {
                match res {
                    Ok(Ok(result)) => slice_results.push(result),
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "slice wave failed");
                        cancel.cancel();
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "slice wave panicked");
                        cancel.cancel();
                    }
                }
            }
            tracing::debug!(count = slice_results.len(), "slice results received");

            // 2. Parallel post-processing per slice with isolated task_graph clones
            struct SlicePostResult {
                task_graph: GoalTaskGraph,
                gates: Vec<crate::runtime::gates::GateResult>,
                changed_files: Vec<String>,
                post_mutation_gates_ran: bool,
                agent_evidence: evidence::GoalAgentRunEvidence,
            }

            let mut post_set = tokio::task::JoinSet::new();
            for (slice, dispatch, agent_evidence) in slice_results {
                let state = state.clone();
                let mut tg = task_graph.clone();
                let vp = verification_proof.clone();
                let ev = agent_evidence.clone();
                let sl = slice.clone();
                let dispatcher = dispatcher.clone();
                post_set.spawn(async move {
                    match dispatch.kind {
                        agent::GoalAgentWaveKind::Initial => {
                            if let Some(task) = task_graph::apply_agent_task_result_by_id(
                                &mut tg,
                                &sl.task_id,
                                &ev,
                                now,
                            ) {
                                dispatcher
                                    .append_execution_events(&state, &task, &ev)
                                    .await?;
                            }
                        }
                        agent::GoalAgentWaveKind::FollowUp => {
                            task_graph::apply_agent_followup_task_results(&mut tg, &ev, now);
                        }
                    }
                    task_graph::apply_agent_proposed_task_mutations(&state, &mut tg, &ev, now)
                        .await?;

                    let succeeded =
                        ev.summary.completed == ev.summary.total && ev.summary.failed == 0;

                    let (gates, _git, changed, ran) = super::run_post_mutation_cycle(
                        &state,
                        &sl.worktree_path,
                        &mut tg,
                        vp,
                        succeeded,
                        &ev,
                        now,
                    )
                    .await?;

                    super::process_slice_delivery_and_review(
                        &state,
                        &mut tg,
                        &sl,
                        succeeded,
                        &sl.worktree_path,
                    )
                    .await?;

                    Ok::<_, anyhow::Error>(SlicePostResult {
                        task_graph: tg,
                        gates,
                        changed_files: changed,
                        post_mutation_gates_ran: ran,
                        agent_evidence: ev,
                    })
                });
            }

            let mut post_results = Vec::with_capacity(post_set.len());
            while let Some(res) = post_set.join_next().await {
                match res {
                    Ok(Ok(result)) => post_results.push(result),
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "slice post-processing failed");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "slice post-processing panicked");
                    }
                }
            }
            tracing::debug!(count = post_results.len(), "post results received");
            for r in post_results.iter() {
                for t in &r.task_graph.tasks {
                    if t.id.starts_with("goal-agent-implement-") {
                        tracing::debug!(task_id = %t.id, status = ?t.status, "post result");
                    }
                }
            }

            // 3. Atomically merge all deltas back into the main task_graph
            let deltas: Vec<GoalTaskGraph> =
                post_results.iter().map(|r| r.task_graph.clone()).collect();
            task_graph::merge_concurrent_slice_task_graphs(&mut task_graph, &deltas);

            let mut all_gates = verification_proof.gates.clone();
            let mut all_changed_files = Vec::new();
            let mut post_mutation_ran = false;
            let mut evidence_refs: Vec<&evidence::GoalAgentRunEvidence> = Vec::new();

            for r in &post_results {
                all_gates = super::merge_gate_results(&all_gates, &r.gates);
                all_changed_files.extend(r.changed_files.iter().cloned());
                post_mutation_ran |= r.post_mutation_gates_ran;
                evidence_refs.push(&r.agent_evidence);
            }

            let git = evidence::detect_git_evidence(project_dir).await;
            let evidence = super::aggregate_agent_evidence(&evidence_refs, goal_id);

            (
                evidence,
                all_gates,
                git,
                all_changed_files,
                post_mutation_ran,
            )
        } else {
            // Serial path (original behavior)
            let exec_project_dir: PathBuf;
            let active_slice: Option<GoalDeliverySlice>;
            if state.slice_execution {
                if let Some(slice) = ready_slices.into_iter().next() {
                    exec_project_dir = slice.worktree_path.clone();
                    active_slice = Some(slice);
                } else {
                    exec_project_dir = project_dir.to_path_buf();
                    active_slice = None;
                }
            } else {
                exec_project_dir = project_dir.to_path_buf();
                active_slice = None;
            }

            let agent_evidence = dispatcher
                .execute_wave(&state, &task_graph, &exec_project_dir, now, &dispatch)
                .await?;
            match dispatch.kind {
                agent::GoalAgentWaveKind::Initial => {
                    if let Some(task) = task_graph::apply_agent_execution_task_result(
                        &mut task_graph,
                        &agent_evidence,
                        now,
                    ) {
                        dispatcher
                            .append_execution_events(&state, &task, &agent_evidence)
                            .await?;
                    }
                }
                agent::GoalAgentWaveKind::FollowUp => {
                    task_graph::apply_agent_followup_task_results(
                        &mut task_graph,
                        &agent_evidence,
                        now,
                    );
                }
            }
            task_graph::apply_agent_proposed_task_mutations(
                &state,
                &mut task_graph,
                &agent_evidence,
                now,
            )
            .await?;

            let agent_execution_succeeded = agent_evidence.summary.completed
                == agent_evidence.summary.total
                && agent_evidence.summary.failed == 0;

            let (proof_gates, proof_git, proof_changed_files, post_mutation_gates_ran) =
                super::run_post_mutation_cycle(
                    &state,
                    &exec_project_dir,
                    &mut task_graph,
                    verification_proof,
                    agent_execution_succeeded,
                    &agent_evidence,
                    now,
                )
                .await?;

            if let Some(slice) = active_slice {
                super::process_slice_delivery_and_review(
                    &state,
                    &mut task_graph,
                    &slice,
                    agent_execution_succeeded,
                    &exec_project_dir,
                )
                .await?;
            }

            (
                agent_evidence,
                proof_gates,
                proof_git,
                proof_changed_files,
                post_mutation_gates_ran,
            )
        };

    let latest_state = crate::runtime::goal::resolve_goal(&state.goal_id).await?;
    let preserve_interrupted_status = matches!(
        latest_state.status,
        GoalStatus::Paused | GoalStatus::Cancelled | GoalStatus::NeedsMoreBudget
    );

    task_graph.save(&state.state_dir).await?;

    let state = super::finalize_execution_state(
        state,
        latest_state,
        preserve_interrupted_status,
        &agent_evidence,
        now,
    )
    .await?;

    let result = super::build_and_persist_execution_proof(
        &state,
        &task_graph,
        proof_gates,
        proof_changed_files,
        proof_git,
        post_mutation_gates_ran,
        now,
    )
    .await;

    if let Err(e) = supervisor::release_goal(&state.goal_id).await {
        tracing::warn!(goal_id = %state.goal_id, error = %e, "Failed to release goal controller PID");
    }

    let phase_duration = tokio::time::Instant::now() - phase_start;
    if let Ok(state) = crate::runtime::goal::resolve_goal(&state.goal_id).await {
        let tracker = crate::cost::tracker::CostTracker::for_goal(
            &state.state_dir,
            state.cost_tracker_path.as_deref(),
        );
        let cost = crate::cost::types::SessionCost {
            session_type: "execute".to_string(),
            name: "goal execute".to_string(),
            started_at: chrono::Utc::now(),
            ended_at: Some(chrono::Utc::now()),
            estimate: crate::cost::estimator::CostEstimate {
                input_tokens: 0,
                output_tokens: 0,
                duration_secs: phase_duration.as_secs(),
                worker_count: actual_worker_count as usize,
                estimated_usd: 0.0,
                tier: crate::cost::estimator::PricingTier::Standard,
            },
            actual_usd: None,
        };
        if let Err(e) = tracker.record(cost).await {
            warn!(error = %e, "Failed to record start cost");
        }
    }

    result
}
