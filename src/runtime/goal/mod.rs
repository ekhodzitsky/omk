use anyhow::Result;
use chrono::Utc;
use std::path::Path;

mod agent;
mod budget;
mod decision;
mod dispatch;
mod evidence;
mod oracle;
mod planner;
mod proof;
mod replay;
mod state;
mod task_graph;
mod verifier;
mod worktree;

// Public API re-exports (preserved for backward compatibility)
pub use budget::{
    add_goal_budget, add_goal_budget_limits, goal_budget, GoalBudgetAdd, GoalBudgetCheckpoint,
    GoalBudgetReport,
};
pub use evidence::GoalGitEvidence;
pub use proof::GoalProof;
pub use replay::{replay_goal, GoalReplay, GoalReplayEntry};
pub use state::{
    CreateGoalOptions, GoalArtifact, GoalFailure, GoalPhase, GoalState, GoalStatus,
    GoalTerminalCriteria, GOALS_DIR, GOAL_AGENT_RUNS_DIR, GOAL_ARTIFACTS_DIR,
    GOAL_BUDGET_CHECKPOINTS_FILE, GOAL_DECISIONS_FILE, GOAL_FAILURE_FILE, GOAL_GATE_ARTIFACTS_DIR,
    GOAL_PRD_FILE, GOAL_PROOF_FILE, GOAL_STATE_FILE, GOAL_TASK_GRAPH_FILE,
    GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
pub use task_graph::{
    GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskGraphSummary, GoalTaskStatus,
};
pub use worktree::{plan_goal_worktree, plan_goal_worktrees, GoalWorktreePlan};

pub async fn create_goal(goal: &str, options: CreateGoalOptions) -> Result<GoalState> {
    planner::create_goal_with_scaffold(goal, options).await
}

pub async fn plan_goal(goal: &str) -> Result<GoalState> {
    planner::create_goal_with_scaffold(
        goal,
        CreateGoalOptions {
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
        },
    )
    .await
}

pub async fn list_goals() -> Result<Vec<GoalState>> {
    let dir = state::goals_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut goals = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            match GoalState::load(&entry.path()).await {
                Ok(state) => goals.push(state),
                Err(error) => tracing::warn!(
                    path = %entry.path().display(),
                    error = %error,
                    "Skipping unreadable goal state"
                ),
            }
        }
    }

    goals.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.goal_id.cmp(&a.goal_id))
    });
    Ok(goals)
}

pub async fn resolve_goal(goal_id: &str) -> Result<GoalState> {
    if goal_id == "latest" {
        let mut goals = list_goals().await?;
        if let Some(goal) = goals.drain(..).next() {
            return Ok(goal);
        }
        anyhow::bail!("No goals found");
    }

    let goal_dir = state::goals_dir().join(goal_id);
    if !goal_dir.exists() {
        anyhow::bail!("Goal '{}' not found", goal_id);
    }
    GoalState::load(&goal_dir).await
}

pub async fn resolve_goal_proof(goal_id: &str) -> Result<GoalProof> {
    let goal = resolve_goal(goal_id).await?;
    GoalProof::load(&goal.state_dir).await
}

pub async fn verify_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = resolve_goal(goal_id).await?;
    ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal verify").await?;
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
    state.save().await?;

    let proof =
        proof::build_verified_proof(&state, &task_graph, gates, changed_files, git, false, now);
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;

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
        .await?;
    budget::append_budget_checkpoint(&state, "verify_completed").await?;

    Ok(proof)
}

pub async fn execute_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = resolve_goal(goal_id).await?;
    ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal execute").await?;
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::Execution;
    state.updated_at = Utc::now();
    state.completed_at = None;
    state.save().await?;

    let verification_proof = verify_goal(goal_id, project_dir).await?;
    let state = resolve_goal(goal_id).await?;
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
    let agent_evidence =
        dispatch::run_goal_agent_task_wave(&state, &task_graph, project_dir, now, &dispatch)
            .await?;
    match dispatch.kind {
        agent::GoalAgentWaveKind::Initial => {
            if let Some(task) =
                task_graph::apply_agent_execution_task_result(&mut task_graph, &agent_evidence, now)
            {
                dispatch::append_agent_execution_task_events(&state, &task, &agent_evidence)
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
    let latest_state = resolve_goal(&state.goal_id).await?;
    let preserve_interrupted_status = matches!(
        latest_state.status,
        GoalStatus::Paused | GoalStatus::Cancelled | GoalStatus::NeedsMoreBudget
    );
    let mut proof_gates = verification_proof.gates;
    let mut proof_git = verification_proof.git;
    let mut proof_changed_files = agent_evidence.changed_files;
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
        verifier::append_gate_events(&state, &proof_gates).await?;
        proof_git = evidence::detect_git_evidence(project_dir).await;
        proof_changed_files = crate::runtime::gates::detect_changed_files(project_dir).await;
        if let Some(task) =
            verifier::apply_local_verification_task_result(&mut task_graph, &proof_gates, now)
        {
            verifier::append_local_verification_task_events(&state, &task).await?;
        }
        post_mutation_gates_ran = true;
    }

    proof::write_json_artifact(
        &state.state_dir.join(state::GOAL_TASK_GRAPH_FILE),
        &task_graph,
    )
    .await?;

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
    state.save().await?;

    let proof = proof::build_verified_proof(
        &state,
        &task_graph,
        proof_gates,
        proof_changed_files,
        proof_git,
        post_mutation_gates_ran,
        now,
    );
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;

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
        .await?;
    budget::append_budget_checkpoint(&state, "execute_completed").await?;

    Ok(proof)
}

pub async fn review_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = resolve_goal(goal_id).await?;
    ensure_goal_can_continue(&state)?;
    budget::ensure_budget_available(&mut state, "goal review").await?;
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::VerificationDesign;
    state.updated_at = Utc::now();
    state.completed_at = None;
    state.save().await?;

    let mut state = resolve_goal(goal_id).await?;
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
    state.save().await?;

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
        .await?;
    budget::append_budget_checkpoint(&state, "review_completed").await?;

    Ok(proof)
}

pub async fn pause_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = resolve_goal(goal_id).await?;
    if matches!(state.status, GoalStatus::Ready | GoalStatus::Cancelled) {
        anyhow::bail!(
            "Goal '{}' is terminal ({}) and cannot be paused",
            state.goal_id,
            state.status
        );
    }

    let now = Utc::now();
    state.status = GoalStatus::Paused;
    state.updated_at = now;
    state.completed_at = None;
    state.save().await?;
    append_goal_lifecycle_event(&state, crate::runtime::events::EventKind::GoalPaused).await?;
    budget::append_budget_checkpoint(&state, "goal_paused").await?;
    Ok(state)
}

pub async fn resume_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = resolve_goal(goal_id).await?;
    if state.status != GoalStatus::Paused {
        anyhow::bail!(
            "Goal '{}' is not paused (status: {})",
            state.goal_id,
            state.status
        );
    }

    let now = Utc::now();
    state.status = GoalStatus::NotReady;
    state.updated_at = now;
    state.completed_at = None;
    state.save().await?;
    append_goal_lifecycle_event(&state, crate::runtime::events::EventKind::GoalResumed).await?;
    budget::append_budget_checkpoint(&state, "goal_resumed").await?;
    Ok(state)
}

pub async fn cancel_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = resolve_goal(goal_id).await?;
    let now = Utc::now();
    state.status = GoalStatus::Cancelled;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.failure = Some(GoalFailure {
        reason: "cancelled by user".to_string(),
        recorded_at: now,
    });
    state.save().await?;

    let failure_json = serde_json::to_string_pretty(&state)?;
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(state::GOAL_FAILURE_FILE),
        failure_json.as_bytes(),
    )
    .await?;

    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let run_id = crate::runtime::events::RunId(state.goal_id.clone());
    let interrupted = crate::runtime::events::Event::new(
        run_id.clone(),
        crate::runtime::events::EventKind::ManualInterrupt,
    )
    .with_actor("omk-cli");
    let failed =
        crate::runtime::events::EventBuilder::new(run_id).run_failed("cancelled by user")?;
    writer.append_many(&[interrupted, failed]).await?;
    budget::append_budget_checkpoint(&state, "goal_cancelled").await?;

    Ok(state)
}

fn ensure_goal_not_paused(state: &GoalState) -> Result<()> {
    if state.status == GoalStatus::Paused {
        anyhow::bail!(
            "Goal '{}' is paused; run `omk goal resume {}` before continuing",
            state.goal_id,
            state.goal_id
        );
    }
    Ok(())
}

fn ensure_goal_can_continue(state: &GoalState) -> Result<()> {
    ensure_goal_not_paused(state)?;
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

async fn append_goal_lifecycle_event(
    state: &GoalState,
    kind: crate::runtime::events::EventKind,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let event = crate::runtime::events::Event::new(
        crate::runtime::events::RunId(state.goal_id.clone()),
        kind,
    )
    .with_actor("omk-cli")
    .with_payload(serde_json::json!({
        "status": state.status.to_string(),
        "phase": state.phase.to_string(),
        "updated_at": state.updated_at,
    }))?;
    writer.append(&event).await
}
