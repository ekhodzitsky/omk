use std::path::Path;

use crate::runtime::goal::proof::{build_verified_proof, GoalProof};
use crate::runtime::goal::state::{
    FileSystemGoalStateStore, GoalPhase, GoalState, GoalStateStore, GoalStatus,
};
use crate::runtime::goal::task_graph::GoalTaskGraph;
use crate::runtime::goal::{budget, evidence, proof, state, verifier};

pub async fn review_goal(goal_id: &str, project_dir: &Path) -> anyhow::Result<GoalProof> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    super::ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal review").await?;
    let phase_start = tokio::time::Instant::now();
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::VerificationDesign;
    state.updated_at = chrono::Utc::now();
    state.completed_at = None;
    FileSystemGoalStateStore::new().save(&state).await?;

    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let prior_proof = GoalProof::load(&state.state_dir).await?;
    let now = chrono::Utc::now();

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

    let proof = build_verified_proof(
        &state,
        &task_graph,
        prior_proof.gates,
        prior_proof.changed_files,
        prior_proof.git,
        prior_proof.post_mutation_gates_ran,
        now,
    );
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;
    super::append_proof_event(&state, &proof).await?;
    budget::append_budget_checkpoint(&state, "review_completed").await?;

    let phase_duration = tokio::time::Instant::now() - phase_start;
    if let Ok(tracker) = budget::init_goal_cost_tracker(&state) {
        let cost = crate::cost::types::SessionCost {
            session_type: "review".to_string(),
            name: "goal review".to_string(),
            started_at: chrono::Utc::now(),
            ended_at: Some(chrono::Utc::now()),
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

pub(crate) async fn run_post_mutation_cycle(
    state: &GoalState,
    project_dir: &Path,
    task_graph: &mut GoalTaskGraph,
    verification_proof: GoalProof,
    agent_execution_succeeded: bool,
    agent_evidence: &evidence::GoalAgentRunEvidence,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<(
    Vec<crate::runtime::gates::GateResult>,
    Option<crate::runtime::goal::evidence::GoalGitEvidence>,
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
