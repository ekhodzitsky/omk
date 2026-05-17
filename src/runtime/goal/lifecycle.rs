use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use super::proof::GoalProof;
use super::state::{GoalPhase, GoalState, GoalStatus};
use super::task_graph::{ready_delivery_slices, GoalDeliverySlice, GoalTaskGraph, GoalTaskStatus};
use super::{agent, budget, dispatch, evidence, proof, review, state, task_graph, verifier};
use crate::runtime::goal::state::{FileSystemGoalStateStore, GoalStateStore};

pub async fn verify_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    verify_goal_with_slices(goal_id, project_dir, None).await
}

pub async fn verify_goal_with_slices(
    goal_id: &str,
    project_dir: &Path,
    slices: Option<&[GoalDeliverySlice]>,
) -> Result<GoalProof> {
    let mut state = super::resolve_goal(goal_id).await?;
    ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal verify").await?;
    let phase_start = tokio::time::Instant::now();
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let gate_config = crate::runtime::gates::load_or_detect_gates(project_dir).await;
    let gate_artifacts = state
        .state_dir
        .join(state::GOAL_ARTIFACTS_DIR)
        .join(state::GOAL_GATE_ARTIFACTS_DIR);
    let mut gates = crate::runtime::gates::run_gates_with_evidence(
        &gate_config,
        project_dir,
        Some(&gate_artifacts),
    )
    .await;
    let mut changed_files = crate::runtime::gates::detect_changed_files(project_dir).await;

    // Optionally run gates on slice worktrees in parallel
    if let Some(slices) = slices {
        let mut gate_set = tokio::task::JoinSet::new();
        for slice in slices {
            let wp = slice.worktree_path.clone();
            let cfg = gate_config.clone();
            let ga = gate_artifacts.join("slice").join(&slice.task_id);
            gate_set.spawn(async move {
                let slice_gates =
                    crate::runtime::gates::run_gates_with_evidence(&cfg, &wp, Some(&ga)).await;
                let slice_changed = crate::runtime::gates::detect_changed_files(&wp).await;
                (slice_gates, slice_changed)
            });
        }
        while let Some(res) = gate_set.join_next().await {
            match res {
                Ok((slice_gates, slice_changed)) => {
                    gates = merge_gate_results(&gates, &slice_gates);
                    changed_files.extend(slice_changed);
                }
                Err(e) => {
                    tracing::warn!("slice gate task failed: {e}");
                }
            }
        }
        changed_files.sort();
        changed_files.dedup();
    }

    let now = Utc::now();

    verifier::append_gate_events(&state, &gates).await?;
    let git = evidence::detect_git_evidence(project_dir).await;
    let updated_task = verifier::apply_local_verification_task_result(&mut task_graph, &gates, now);
    if let Some(task) = &updated_task {
        verifier::append_local_verification_task_events(&state, task).await?;
    }
    proof::write_json_artifact(
        &state.state_dir.join(state::GOAL_TASK_GRAPH_FILE),
        &task_graph,
    )
    .await?;

    state.status = GoalStatus::NotReady;
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    FileSystemGoalStateStore::new().save(&state).await?;

    let proof =
        proof::build_verified_proof(&state, &task_graph, gates, changed_files, git, false, now);
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;
    append_proof_event(&state, &proof).await?;
    budget::append_budget_checkpoint(&state, "verify_completed").await?;

    let phase_duration = tokio::time::Instant::now() - phase_start;
    if let Ok(tracker) = budget::init_goal_cost_tracker(&state) {
        let worker_count = slices.map(|s| s.len()).unwrap_or(1);
        let cost = crate::cost::types::SessionCost {
            session_type: "verify".to_string(),
            name: "goal verify".to_string(),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            estimate: crate::cost::estimator::CostEstimate {
                input_tokens: 0,
                output_tokens: 0,
                duration_secs: phase_duration.as_secs(),
                worker_count,
                estimated_usd: 0.0,
                tier: crate::cost::estimator::PricingTier::Standard,
            },
            actual_usd: None,
        };
        let _ = tracker.record(cost).await;
    }

    Ok(proof)
}

pub async fn execute_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let dispatcher = dispatch::DefaultGoalDispatcher;
    execute_goal_with_dispatcher(goal_id, project_dir, &dispatcher).await
}

pub async fn execute_goal_with_dispatcher<D: dispatch::GoalDispatcher + Clone + 'static>(
    goal_id: &str,
    project_dir: &Path,
    dispatcher: &D,
) -> Result<GoalProof> {
    let mut state = super::resolve_goal(goal_id).await?;
    ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal execute").await?;
    let phase_start = tokio::time::Instant::now();
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::Execution;
    state.updated_at = Utc::now();
    state.completed_at = None;
    FileSystemGoalStateStore::new().save(&state).await?;

    let now = Utc::now();
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
        verify_goal_with_slices(goal_id, project_dir, Some(&ready_slices)).await?
    } else {
        verify_goal_with_slices(goal_id, project_dir, None).await?
    };

    let state = super::resolve_goal(goal_id).await?;
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
                        tracing::warn!("slice wave failed: {e}");
                        cancel.cancel();
                    }
                    Err(e) => {
                        tracing::warn!("slice wave panicked: {e}");
                        cancel.cancel();
                    }
                }
            }
            eprintln!("DEBUG slice_results.len()={}", slice_results.len());

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

                    let (gates, _git, changed, ran) = run_post_mutation_cycle(
                        &state,
                        &sl.worktree_path,
                        &mut tg,
                        vp,
                        succeeded,
                        &ev,
                        now,
                    )
                    .await?;

                    process_slice_delivery_and_review(
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
                        tracing::warn!("slice post-processing failed: {e}");
                    }
                    Err(e) => {
                        tracing::warn!("slice post-processing panicked: {e}");
                    }
                }
            }
            eprintln!("DEBUG post_results.len()={}", post_results.len());
            for (i, r) in post_results.iter().enumerate() {
                for t in &r.task_graph.tasks {
                    if t.id.starts_with("goal-agent-implement-") {
                        eprintln!("DEBUG post_result[{i}] task {} status={:?}", t.id, t.status);
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
                all_gates = merge_gate_results(&all_gates, &r.gates);
                all_changed_files.extend(r.changed_files.iter().cloned());
                post_mutation_ran |= r.post_mutation_gates_ran;
                evidence_refs.push(&r.agent_evidence);
            }

            let git = evidence::detect_git_evidence(project_dir).await;
            let evidence = aggregate_agent_evidence(&evidence_refs, goal_id);

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
                run_post_mutation_cycle(
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
                process_slice_delivery_and_review(
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

    let latest_state = super::resolve_goal(&state.goal_id).await?;
    let preserve_interrupted_status = matches!(
        latest_state.status,
        GoalStatus::Paused | GoalStatus::Cancelled | GoalStatus::NeedsMoreBudget
    );

    proof::write_json_artifact(
        &state.state_dir.join(state::GOAL_TASK_GRAPH_FILE),
        &task_graph,
    )
    .await?;

    let state = finalize_execution_state(
        state,
        latest_state,
        preserve_interrupted_status,
        &agent_evidence,
        now,
    )
    .await?;

    let result = build_and_persist_execution_proof(
        &state,
        &task_graph,
        proof_gates,
        proof_changed_files,
        proof_git,
        post_mutation_gates_ran,
        now,
    )
    .await;

    let phase_duration = tokio::time::Instant::now() - phase_start;
    if let Ok(state) = super::resolve_goal(goal_id).await {
        if let Ok(tracker) = budget::init_goal_cost_tracker(&state) {
            let cost = crate::cost::types::SessionCost {
                session_type: "execute".to_string(),
                name: "goal execute".to_string(),
                started_at: Utc::now(),
                ended_at: Some(Utc::now()),
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
            let _ = tracker.record(cost).await;
        }
    }

    result
}

async fn run_post_mutation_cycle(
    state: &GoalState,
    project_dir: &Path,
    task_graph: &mut GoalTaskGraph,
    verification_proof: GoalProof,
    agent_execution_succeeded: bool,
    agent_evidence: &evidence::GoalAgentRunEvidence,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(
    Vec<crate::runtime::gates::GateResult>,
    Option<super::evidence::GoalGitEvidence>,
    Vec<String>,
    bool,
)> {
    let mut proof_gates = verification_proof.gates;
    let mut proof_git = verification_proof.git;
    let mut proof_changed_files = agent_evidence.changed_files.clone();
    let mut post_mutation_gates_ran = false;

    if agent_execution_succeeded && !proof_changed_files.is_empty() {
        let gate_config = crate::runtime::gates::load_or_detect_gates(project_dir).await;
        let gate_artifacts = state
            .state_dir
            .join(state::GOAL_ARTIFACTS_DIR)
            .join(state::GOAL_GATE_ARTIFACTS_DIR)
            .join("post-mutation");
        proof_gates = crate::runtime::gates::run_gates_with_evidence(
            &gate_config,
            project_dir,
            Some(&gate_artifacts),
        )
        .await;
        verifier::append_gate_events(state, &proof_gates).await?;
        proof_git = evidence::detect_git_evidence(project_dir).await;
        proof_changed_files = crate::runtime::gates::detect_changed_files(project_dir).await;
        if let Some(task) =
            verifier::apply_local_verification_task_result(task_graph, &proof_gates, now)
        {
            verifier::append_local_verification_task_events(state, &task).await?;
        }
        post_mutation_gates_ran = true;
    }

    Ok((
        proof_gates,
        proof_git,
        proof_changed_files,
        post_mutation_gates_ran,
    ))
}

async fn process_slice_delivery_and_review(
    state: &GoalState,
    task_graph: &mut GoalTaskGraph,
    slice: &GoalDeliverySlice,
    agent_execution_succeeded: bool,
    exec_project_dir: &Path,
) -> Result<()> {
    if agent_execution_succeeded
        && state.delivery_policy != crate::runtime::goal::GoalDeliveryPolicy::Local
    {
        let base_branch = super::control::resolve_base_branch(exec_project_dir).await;
        let delivery_options = super::delivery::SlicePrDeliveryOptions {
            policy: state.delivery_policy,
            dry_run: false,
            base_branch,
        };
        let delivery =
            super::delivery::deliver_slice_pr(exec_project_dir, slice, state, delivery_options)
                .await;

        let review = review::review_slice(slice, state, task_graph, exec_project_dir).await;

        let (slice_status, _feedback) = match (delivery, review) {
            (Ok(d), Ok(r)) => {
                let anti_slop_confidence = review::anti_slop_confidence(&r.artifacts);
                let anti_slop_actionable =
                    anti_slop_confidence > review::ANTI_SLOP_ACTIONABLE_THRESHOLD;

                let mut extra = serde_json::Map::new();

                let status = if anti_slop_actionable {
                    let changed_files =
                        crate::runtime::gates::detect_changed_files(exec_project_dir).await;
                    let feedback_summary = r
                        .artifacts
                        .iter()
                        .filter(|a| !a.passed)
                        .map(|a| format!("{}: {}", a.kind, a.feedback))
                        .collect::<Vec<_>>()
                        .join("; ");

                    if let Some(cleanup_task_id) = task_graph::spawn_cleanup_task(
                        task_graph,
                        &slice.task_id,
                        &feedback_summary,
                        &changed_files,
                        Utc::now(),
                    ) {
                        let writer = crate::runtime::events::EventWriter::new(
                            state.state_dir.join(crate::runtime::config::EVENTS_FILE),
                        );
                        let event = crate::runtime::events::Event::new(
                            crate::runtime::events::RunId(state.goal_id.clone()),
                            crate::runtime::events::EventKind::TaskGraphMutated,
                        )
                        .with_actor(state::GOAL_CONTROLLER_ACTOR)
                        .with_payload(
                            crate::runtime::events::TaskGraphMutationPayload {
                                action: "task_added".to_string(),
                                source: "anti_slop_cleanup".to_string(),
                                task_id: crate::runtime::events::TaskId(cleanup_task_id),
                                task_graph_path: PathBuf::from(state::GOAL_TASK_GRAPH_FILE),
                                proposal_path: PathBuf::new(),
                                total_tasks_after: task_graph.tasks.len(),
                            },
                        )?;
                        writer.append(&event).await?;
                    }

                    extra.insert(
                        "review_feedback".to_string(),
                        serde_json::Value::String(format!(
                            "Anti-slop confidence {anti_slop_confidence:.2} exceeds threshold. Cleanup task spawned for slice {}.",
                            slice.task_id
                        )),
                    );
                    task_graph::GoalTaskDeliveryStatus::Blocked
                } else if !r.passed {
                    if let Some(ref fb) = r.feedback {
                        extra.insert(
                            "review_feedback".to_string(),
                            serde_json::Value::String(fb.clone()),
                        );
                    }
                    if let Some(task) = task_graph.tasks.iter_mut().find(|t| t.id == slice.task_id)
                    {
                        task.status = GoalTaskStatus::Pending;
                        task.completed_at = None;
                        if let Some(ref fb) = r.feedback {
                            task.description =
                                format!("{}\n\n[review-feedback] {}", task.description, fb);
                        }
                    }
                    task_graph::GoalTaskDeliveryStatus::Blocked
                } else {
                    task_graph::GoalTaskDeliveryStatus::Delivered
                };
                (
                    task_graph::GoalTaskDeliveryMetadataUpdate {
                        status: Some(status),
                        pr_url: d.pr_url,
                        commit_sha: d.commit_sha,
                        extra,
                        ..Default::default()
                    },
                    None::<String>,
                )
            }
            (Err(e), _) | (_, Err(e)) => {
                let mut extra = serde_json::Map::new();
                let error_msg = format!("Slice delivery/review error: {e}");
                extra.insert(
                    "review_feedback".to_string(),
                    serde_json::Value::String(error_msg.clone()),
                );
                if let Some(task) = task_graph.tasks.iter_mut().find(|t| t.id == slice.task_id) {
                    task.status = GoalTaskStatus::Pending;
                    task.completed_at = None;
                    task.description =
                        format!("{}\n\n[review-feedback] {}", task.description, error_msg);
                }
                (
                    task_graph::GoalTaskDeliveryMetadataUpdate {
                        status: Some(task_graph::GoalTaskDeliveryStatus::Blocked),
                        extra,
                        ..Default::default()
                    },
                    None,
                )
            }
        };

        task_graph::update_goal_task_delivery_metadata(
            &state.state_dir,
            &slice.task_id,
            slice_status,
        )
        .await?;
    } else {
        let slice_status = if agent_execution_succeeded {
            task_graph::GoalTaskDeliveryStatus::Delivered
        } else {
            task_graph::GoalTaskDeliveryStatus::Blocked
        };
        task_graph::update_goal_task_delivery_metadata(
            &state.state_dir,
            &slice.task_id,
            task_graph::GoalTaskDeliveryMetadataUpdate {
                status: Some(slice_status),
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}

async fn finalize_execution_state(
    state: GoalState,
    latest_state: GoalState,
    preserve_interrupted_status: bool,
    agent_evidence: &evidence::GoalAgentRunEvidence,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<GoalState> {
    let mut state = if preserve_interrupted_status {
        latest_state
    } else {
        state
    };
    evidence::record_artifact_path_once(
        &mut state,
        "agent_run",
        agent_evidence.run_path.clone(),
        now,
    );
    if !preserve_interrupted_status {
        state.status = GoalStatus::NotReady;
        state.phase = GoalPhase::Proof;
        state.completed_at = Some(now);
    }
    state.updated_at = now;
    FileSystemGoalStateStore::new().save(&state).await?;
    Ok(state)
}

async fn build_and_persist_execution_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    proof_gates: Vec<crate::runtime::gates::GateResult>,
    proof_changed_files: Vec<String>,
    proof_git: Option<super::evidence::GoalGitEvidence>,
    post_mutation_gates_ran: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<GoalProof> {
    let proof = proof::build_verified_proof(
        state,
        task_graph,
        proof_gates,
        proof_changed_files,
        proof_git,
        post_mutation_gates_ran,
        now,
    );
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;
    append_proof_event(state, &proof).await?;
    budget::append_budget_checkpoint(state, "execute_completed").await?;
    Ok(proof)
}

pub async fn review_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = super::resolve_goal(goal_id).await?;
    ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal review").await?;
    let phase_start = tokio::time::Instant::now();
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::VerificationDesign;
    state.updated_at = Utc::now();
    state.completed_at = None;
    FileSystemGoalStateStore::new().save(&state).await?;

    let mut state = super::resolve_goal(goal_id).await?;
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let prior_proof = GoalProof::load(&state.state_dir).await?;
    let now = Utc::now();

    let review_evidence =
        verifier::write_goal_review_evidence(&state, &task_graph, &prior_proof, project_dir, now)
            .await?;
    let mut updated_tasks = Vec::new();
    if let Some(task) =
        verifier::apply_goal_review_task_result(&mut task_graph, &review_evidence, now)
    {
        updated_tasks.push(task);
    }
    if let Some(task) =
        verifier::apply_goal_security_review_task_result(&mut task_graph, &review_evidence, now)
    {
        updated_tasks.push(task);
    }
    verifier::append_goal_review_task_events(&state, &updated_tasks).await?;
    proof::write_json_artifact(
        &state.state_dir.join(state::GOAL_TASK_GRAPH_FILE),
        &task_graph,
    )
    .await?;

    evidence::record_artifact_path_once(
        &mut state,
        "review",
        review_evidence.review_path.clone(),
        now,
    );
    evidence::record_artifact_path_once(
        &mut state,
        "security_review",
        review_evidence.security_review_path.clone(),
        now,
    );
    state.status = GoalStatus::NotReady;
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    FileSystemGoalStateStore::new().save(&state).await?;

    let proof = proof::build_verified_proof(
        &state,
        &task_graph,
        prior_proof.gates,
        prior_proof.changed_files,
        prior_proof.git,
        prior_proof.post_mutation_gates_ran,
        now,
    );
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;
    append_proof_event(&state, &proof).await?;
    budget::append_budget_checkpoint(&state, "review_completed").await?;

    let phase_duration = tokio::time::Instant::now() - phase_start;
    if let Ok(tracker) = budget::init_goal_cost_tracker(&state) {
        let cost = crate::cost::types::SessionCost {
            session_type: "review".to_string(),
            name: "goal review".to_string(),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            estimate: crate::cost::estimator::CostEstimate {
                input_tokens: 0,
                output_tokens: 0,
                duration_secs: phase_duration.as_secs(),
                worker_count: 1,
                estimated_usd: 0.0,
                tier: crate::cost::estimator::PricingTier::Standard,
            },
            actual_usd: None,
        };
        let _ = tracker.record(cost).await;
    }

    Ok(proof)
}

/// Merge gate results from multiple worktrees. A gate passes only if it passes
/// in ALL result sets where it appears.
fn merge_gate_results(
    base: &[crate::runtime::gates::GateResult],
    extra: &[crate::runtime::gates::GateResult],
) -> Vec<crate::runtime::gates::GateResult> {
    use std::collections::HashMap;

    let mut merged: HashMap<String, crate::runtime::gates::GateResult> =
        base.iter().map(|g| (g.name.clone(), g.clone())).collect();
    for g in extra {
        merged
            .entry(g.name.clone())
            .and_modify(|existing| {
                existing.passed &= g.passed;
                if !g.passed {
                    existing.stderr = g.stderr.to_string();
                }
            })
            .or_insert_with(|| g.clone());
    }
    merged.into_values().collect()
}

/// Aggregate evidence from multiple concurrent slices into a single
/// `GoalAgentRunEvidence` representing the whole swarm.
fn aggregate_agent_evidence(
    slices: &[&evidence::GoalAgentRunEvidence],
    goal_id: &str,
) -> evidence::GoalAgentRunEvidence {
    use crate::runtime::scheduler::runner::RunSummary;

    let mut combined = evidence::GoalAgentRunEvidence {
        summary: RunSummary {
            run_id: format!("{goal_id}-concurrent"),
            completed: slices.iter().map(|e| e.summary.completed).sum(),
            failed: slices.iter().map(|e| e.summary.failed).sum(),
            cancelled: slices.iter().map(|e| e.summary.cancelled).sum(),
            total: slices.iter().map(|e| e.summary.total).sum(),
        },
        run_path: PathBuf::new(),
        task_policy_path: PathBuf::new(),
        agent_task_proposals_path: PathBuf::new(),
        worker_outbox_path: PathBuf::new(),
        wire_events_path: PathBuf::new(),
        mutation_diff_path: PathBuf::new(),
        changed_files_path: PathBuf::new(),
        changed_files: Vec::new(),
        accepted_task_count: 0,
        rejected_task_count: 0,
        accepted_task_ids: Vec::new(),
        agent_proposed_tasks: Vec::new(),
        worker_results: Vec::new(),
        worker_summary: None,
    };

    for ev in slices {
        combined
            .changed_files
            .extend(ev.changed_files.iter().cloned());
        combined.accepted_task_count += ev.accepted_task_count;
        combined.rejected_task_count += ev.rejected_task_count;
        combined
            .accepted_task_ids
            .extend(ev.accepted_task_ids.iter().cloned());
        combined
            .agent_proposed_tasks
            .extend(ev.agent_proposed_tasks.iter().cloned());
        combined
            .worker_results
            .extend(ev.worker_results.iter().cloned());
        if combined.worker_summary.is_none() && ev.worker_summary.is_some() {
            combined.worker_summary = ev.worker_summary.clone();
        }
        if combined.run_path.as_os_str().is_empty() && !ev.run_path.as_os_str().is_empty() {
            combined.run_path = ev.run_path.clone();
        }
        if combined.task_policy_path.as_os_str().is_empty()
            && !ev.task_policy_path.as_os_str().is_empty()
        {
            combined.task_policy_path = ev.task_policy_path.clone();
        }
        if combined.agent_task_proposals_path.as_os_str().is_empty()
            && !ev.agent_task_proposals_path.as_os_str().is_empty()
        {
            combined.agent_task_proposals_path = ev.agent_task_proposals_path.clone();
        }
        if combined.worker_outbox_path.as_os_str().is_empty()
            && !ev.worker_outbox_path.as_os_str().is_empty()
        {
            combined.worker_outbox_path = ev.worker_outbox_path.clone();
        }
        if combined.wire_events_path.as_os_str().is_empty()
            && !ev.wire_events_path.as_os_str().is_empty()
        {
            combined.wire_events_path = ev.wire_events_path.clone();
        }
        if combined.mutation_diff_path.as_os_str().is_empty()
            && !ev.mutation_diff_path.as_os_str().is_empty()
        {
            combined.mutation_diff_path = ev.mutation_diff_path.clone();
        }
        if combined.changed_files_path.as_os_str().is_empty()
            && !ev.changed_files_path.as_os_str().is_empty()
        {
            combined.changed_files_path = ev.changed_files_path.clone();
        }
    }

    combined.changed_files.sort();
    combined.changed_files.dedup();
    combined
}

fn ensure_goal_can_continue(state: &GoalState) -> Result<()> {
    if state.status == GoalStatus::Paused {
        anyhow::bail!(
            "Goal '{}' is paused; run `omk goal resume {}` before continuing",
            state.goal_id,
            state.goal_id
        );
    }
    if state.status == GoalStatus::BlockedOnHuman {
        let reason = state
            .failure
            .as_ref()
            .map(|failure| failure.reason.as_str())
            .unwrap_or("human decision required");
        anyhow::bail!("Goal '{}' is blocked_on_human: {reason}", state.goal_id);
    }
    Ok(())
}

async fn append_proof_event(state: &GoalState, proof: &GoalProof) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let builder = crate::runtime::events::EventBuilder::new(crate::runtime::events::RunId(
        state.goal_id.clone(),
    ));
    writer
        .append(&builder.proof_written(
            &state.state_dir.join(state::GOAL_PROOF_FILE),
            &proof.status.to_string(),
        )?)
        .await
}
