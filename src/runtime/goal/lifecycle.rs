use anyhow::Result;
use chrono::Utc;
use std::path::Path;

use super::proof::GoalProof;
use super::state::{GoalPhase, GoalState, GoalStatus};
use super::task_graph::{GoalTaskGraph, GoalTaskStatus};
use super::{agent, budget, dispatch, evidence, proof, state, task_graph, verifier};
use crate::runtime::goal::state::{FileSystemGoalStateStore, GoalStateStore};

pub async fn verify_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
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
    let gates = crate::runtime::gates::run_gates_with_evidence(
        &gate_config,
        project_dir,
        Some(&gate_artifacts),
    )
    .await;
    let changed_files = crate::runtime::gates::detect_changed_files(project_dir).await;
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
        let cost = crate::cost::types::SessionCost {
            session_type: "verify".to_string(),
            name: "goal verify".to_string(),
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

pub async fn execute_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let dispatcher = dispatch::DefaultGoalDispatcher;
    execute_goal_with_dispatcher(goal_id, project_dir, &dispatcher).await
}

async fn execute_goal_with_dispatcher<D: dispatch::GoalDispatcher>(
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

    let verification_proof = verify_goal(goal_id, project_dir).await?;
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

    let now = Utc::now();
    let agent_evidence = dispatcher
        .execute_wave(&state, &task_graph, project_dir, now, &dispatch)
        .await?;
    match dispatch.kind {
        agent::GoalAgentWaveKind::Initial => {
            if let Some(task) =
                task_graph::apply_agent_execution_task_result(&mut task_graph, &agent_evidence, now)
            {
                dispatcher
                    .append_execution_events(&state, &task, &agent_evidence)
                    .await?;
            }
        }
        agent::GoalAgentWaveKind::FollowUp => {
            task_graph::apply_agent_followup_task_results(&mut task_graph, &agent_evidence, now);
        }
    }
    task_graph::apply_agent_proposed_task_mutations(&state, &mut task_graph, &agent_evidence, now)
        .await?;

    let agent_execution_succeeded = agent_evidence.summary.completed
        == agent_evidence.summary.total
        && agent_evidence.summary.failed == 0;
    let latest_state = super::resolve_goal(&state.goal_id).await?;
    let preserve_interrupted_status = matches!(
        latest_state.status,
        GoalStatus::Paused | GoalStatus::Cancelled | GoalStatus::NeedsMoreBudget
    );

    let (proof_gates, proof_git, proof_changed_files, post_mutation_gates_ran) =
        run_post_mutation_cycle(
            &state,
            project_dir,
            &mut task_graph,
            verification_proof,
            agent_execution_succeeded,
            &agent_evidence,
            now,
        )
        .await?;

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
                    worker_count: 1,
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
