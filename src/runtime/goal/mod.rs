use anyhow::Result;
use chrono::Utc;

mod agent;
mod budget;
mod control;
mod decision;
mod delivery;
mod dispatch;
mod evidence;
mod integration;
mod lifecycle;
mod open_pr;
mod oracle;
mod planner;
mod proof;
mod replay;
mod state;
mod task_graph;
mod types;
mod verifier;
mod worktree;

pub use agent::{check_task_path_policy, GoalAgentTaskProposal};

// Public API re-exports (preserved for backward compatibility)
pub use budget::{
    add_goal_budget, add_goal_budget_limits, evaluate_task_budget, goal_budget, GoalBudgetAdd,
    GoalBudgetCheckpoint, GoalBudgetReport, PerTaskBudgetSnapshot,
};
pub(crate) use control::run_goal_until_ready;
pub use control::{cancel_goal, pause_goal, resume_goal};
pub use delivery::{
    deliver_goal_open_pr_with_client, open_goal_pr_with_client, GoalDeliveryPolicy,
    GoalGithubPrClient, GoalGithubPrCommandClient, GoalGithubPrDeliveryOptions,
    GoalGithubPrDeliveryOutcome, GoalGithubPrMutation, GoalGithubPrOperation, GoalGithubPrRequest,
};
pub use evidence::GoalGitEvidence;
pub(crate) use integration::{accept_goal, reject_goal};
pub use lifecycle::{execute_goal, review_goal, verify_goal};
pub(crate) use open_pr::render_goal_open_pr;
pub use open_pr::GoalOpenPrDraft;
pub use oracle::GoalKind;
pub use proof::GoalProof;
pub use replay::{replay_goal, GoalReplay, GoalReplayEntry};
pub use state::{
    CreateGoalOptions, GoalArtifact, GoalFailure, GoalPhase, GoalState, GoalStateError, GoalStatus,
    GoalTerminalCriteria, GOALS_DIR, GOAL_AGENT_RUNS_DIR, GOAL_ARTIFACTS_DIR,
    GOAL_BUDGET_CHECKPOINTS_FILE, GOAL_DECISIONS_FILE, GOAL_FAILURE_FILE, GOAL_GATE_ARTIFACTS_DIR,
    GOAL_PRD_FILE, GOAL_PROOF_FILE, GOAL_STATE_FILE, GOAL_TASK_GRAPH_FILE,
    GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
pub use task_graph::{
    load_goal_task_delivery_records, read_goal_task_delivery_metadata,
    update_goal_task_delivery_metadata, GoalTask, GoalTaskDeliveryMetadata,
    GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryRecord, GoalTaskDeliveryStatus,
    GoalTaskEvidence, GoalTaskGraph, GoalTaskGraphSummary, GoalTaskStatus,
};
pub(crate) use types::GoalRunUntilReadyOutcome;
pub use types::{GoalBudget, GoalId};
pub use worktree::{
    detect_goal_merge_conflicts, materialize_goal_worktrees, plan_goal_worktree,
    plan_goal_worktrees, GoalMergeConflictCheckRequest, GoalMergeConflictEvidence,
    GoalWorktreeMaterializeOutcome, GoalWorktreeMaterializeRequest, GoalWorktreePlan,
};

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
        anyhow::bail!(
            "No goals found in {}\n\nCreate one with:\n  omk goal run \"<engineering goal>\"",
            state::goals_dir().display()
        );
    }

    let goal_dir = state::goals_dir().join(goal_id);
    if !goal_dir.exists() {
        anyhow::bail!(
            "Goal '{goal_id}' not found.\n\nList existing goals:\n  omk goal list\n\nState directory: {}",
            state::goals_dir().display()
        );
    }
    GoalState::load(&goal_dir).await
}

pub(crate) use state::parse_budget_duration;

pub async fn resolve_goal_proof(goal_id: &str) -> Result<GoalProof> {
    let goal = resolve_goal(goal_id).await?;
    match GoalProof::load(&goal.state_dir).await {
        Ok(mut proof) => {
            proof::reconcile_goal_proof_with_state(&mut proof, &goal);
            Ok(proof)
        }
        Err(error) => {
            let (task_graph, task_graph_gap) = match GoalTaskGraph::load(&goal.state_dir).await {
                Ok(graph) => (graph, None),
                Err(graph_error) => {
                    let gap = format!(
                        "Task graph could not be loaded while rebuilding proof: {}",
                        graph_error.root_cause()
                    );
                    (
                        GoalTaskGraph {
                            version: 1,
                            goal_id: goal.goal_id.clone(),
                            generated_at: Utc::now(),
                            tasks: Vec::new(),
                        },
                        Some(gap),
                    )
                }
            };
            let git = evidence::detect_git_evidence(&goal.state_dir).await;
            let mut proof = proof::build_scaffold_proof(&goal, &task_graph, git, Utc::now());
            proof.known_gaps.push(format!(
                "Proof file could not be loaded; rebuilt from state: {}",
                error.root_cause()
            ));
            if let Some(gap) = task_graph_gap {
                proof.known_gaps.push(gap);
            }
            proof.recovery_status = Some(format!(
                "recovered: proof rebuilt from state because load failed: {}",
                error.root_cause()
            ));
            Ok(proof)
        }
    }
}
