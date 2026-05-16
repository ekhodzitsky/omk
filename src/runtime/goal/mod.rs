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
mod progress;
mod proof;
mod queries;
mod replay;
mod state;
mod task_graph;
mod types;
mod verifier;
mod worktree;

// Public API
pub use agent::{check_task_path_policy, GoalAgentTaskProposal};
pub use budget::{
    add_goal_budget, add_goal_budget_limits, evaluate_task_budget, goal_budget, GoalBudgetAdd,
    GoalBudgetCheckpoint, GoalBudgetReport, PerTaskBudgetSnapshot,
};
pub use control::{cancel_goal, pause_goal, resume_goal};
pub use delivery::{
    deliver_goal_open_pr_with_client, open_goal_pr_with_client, poll_github_pr_checks,
    GoalDeliveryPolicy, GoalGithubPrClient, GoalGithubPrCommandClient, GoalGithubPrDeliveryOptions,
    GoalGithubPrDeliveryOutcome, GoalGithubPrMutation, GoalGithubPrOperation, GoalGithubPrRequest,
    GoalMergePolicy,
};
pub use evidence::GoalGitEvidence;
pub use lifecycle::{execute_goal, review_goal, verify_goal};
pub use open_pr::GoalOpenPrDraft;
pub use oracle::GoalKind;
pub use progress::{GoalProgressLine, GoalProgressLineKind, GoalProgressSnapshot};
pub use proof::GoalProof;
pub use queries::{list_goals, resolve_goal, resolve_goal_proof};
pub use replay::{replay_goal, GoalReplay, GoalReplayEntry};
pub use state::{
    CreateGoalOptions, FileSystemGoalStateStore, GoalArtifact, GoalFailure, GoalPhase, GoalState,
    GoalStateError, GoalStateStore, GoalStatus, GoalTerminalCriteria, GOALS_DIR,
    GOAL_AGENT_RUNS_DIR, GOAL_ARTIFACTS_DIR, GOAL_BUDGET_CHECKPOINTS_FILE, GOAL_DECISIONS_FILE,
    GOAL_FAILURE_FILE, GOAL_GATE_ARTIFACTS_DIR, GOAL_PRD_FILE, GOAL_PROOF_FILE, GOAL_STATE_FILE,
    GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
pub use task_graph::{
    load_goal_task_delivery_records, plan_goal_delivery_slices, read_goal_task_delivery_metadata,
    record_goal_delivery_slice_plan, update_goal_task_delivery_metadata,
    GoalDeliveryOverlapSerialization, GoalDeliverySlice, GoalDeliverySlicePlan, GoalTask,
    GoalTaskDeliveryMetadata, GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryRecord,
    GoalTaskDeliveryStatus, GoalTaskEvidence, GoalTaskGraph, GoalTaskGraphSummary, GoalTaskStatus,
};
pub use types::{GoalBudget, GoalId};
pub use worktree::{
    detect_goal_merge_conflicts, materialize_goal_worktrees, plan_goal_worktree,
    plan_goal_worktrees, GoalMergeConflictCheckRequest, GoalMergeConflictEvidence,
    GoalWorktreeMaterializeOutcome, GoalWorktreeMaterializeRequest, GoalWorktreePlan,
};

// Internal API
pub(crate) use control::run_goal_until_ready;
pub(crate) use integration::{accept_goal, reject_goal};
pub(crate) use open_pr::render_goal_open_pr;
pub(crate) use state::parse_budget_duration;
pub(crate) use types::GoalRunUntilReadyOutcome;

// Facade functions
pub async fn create_goal(goal: &str, options: CreateGoalOptions) -> anyhow::Result<GoalState> {
    planner::create_goal_with_scaffold(goal, options).await
}

pub async fn plan_goal(goal: &str) -> anyhow::Result<GoalState> {
    planner::create_goal_with_scaffold(
        goal,
        CreateGoalOptions {
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            delivery_policy: GoalDeliveryPolicy::Local,
            merge_policy: GoalMergePolicy::Disabled,
            slice_execution: false,
        },
    )
    .await
}
