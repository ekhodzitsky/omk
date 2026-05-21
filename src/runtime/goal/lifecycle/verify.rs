use std::path::Path;

use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{FileSystemGoalStateStore, GoalStateStore};
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTaskGraph};
use crate::runtime::goal::{budget, evidence, proof, state, verifier};

pub async fn verify_goal(goal_id: &str, project_dir: &Path) -> anyhow::Result<GoalProof> {
    verify_goal_with_slices(goal_id, project_dir, None).await
}

pub async fn verify_goal_with_slices(
    goal_id: &str,
    project_dir: &Path,
    slices: Option<&[GoalDeliverySlice]>,
) -> anyhow::Result<GoalProof> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    super::ensure_goal_can_continue(&state)?;
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
                    gates = super::merge_gate_results(&gates, &slice_gates);
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

    let now = chrono::Utc::now();

    verifier::append_gate_events(&state, &gates).await?;
    let git = evidence::detect_git_evidence(project_dir).await;
    let updated_task = verifier::apply_local_verification_task_result(&mut task_graph, &gates, now);
    if let Some(task) = &updated_task {
        verifier::append_local_verification_task_events(&state, task).await?;
    }
    task_graph.save(&state.state_dir).await?;

    state.status = crate::runtime::goal::state::GoalStatus::NotReady;
    state.phase = crate::runtime::goal::state::GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    FileSystemGoalStateStore::new().save(&state).await?;

    let proof =
        proof::build_verified_proof(&state, &task_graph, gates, changed_files, git, false, now);
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;
    super::append_proof_event(&state, &proof).await?;
    budget::append_budget_checkpoint(&state, "verify_completed").await?;

    let phase_duration = tokio::time::Instant::now() - phase_start;
    if let Ok(tracker) = budget::init_goal_cost_tracker(&state) {
        let worker_count = slices.map(|s| s.len()).unwrap_or(1);
        let cost = crate::cost::types::SessionCost {
            session_type: "verify".to_string(),
            name: "goal verify".to_string(),
            started_at: chrono::Utc::now(),
            ended_at: Some(chrono::Utc::now()),
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
