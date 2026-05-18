use crate::runtime::goal::evidence::GoalAgentRunEvidence;
use crate::runtime::goal::proof::{build_verified_proof, write_json_artifact, GoalProof};
use crate::runtime::goal::state::{
    FileSystemGoalStateStore, GoalPhase, GoalState, GoalStateStore, GoalStatus,
};
use crate::runtime::goal::task_graph::GoalTaskGraph;
use crate::runtime::goal::{budget, state};

pub(crate) async fn finalize_execution_state(
    state: GoalState,
    latest_state: GoalState,
    preserve_interrupted_status: bool,
    agent_evidence: &GoalAgentRunEvidence,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<GoalState> {
    let mut state = if preserve_interrupted_status {
        latest_state
    } else {
        state
    };
    crate::runtime::goal::evidence::record_artifact_path_once(
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

pub(crate) async fn build_and_persist_execution_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    proof_gates: Vec<crate::runtime::gates::GateResult>,
    proof_changed_files: Vec<String>,
    proof_git: Option<crate::runtime::goal::evidence::GoalGitEvidence>,
    post_mutation_gates_ran: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<GoalProof> {
    let proof = build_verified_proof(
        state,
        task_graph,
        proof_gates,
        proof_changed_files,
        proof_git,
        post_mutation_gates_ran,
        now,
    );
    write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;
    super::append_proof_event(state, &proof).await?;
    budget::append_budget_checkpoint(state, "execute_completed").await?;
    Ok(proof)
}
