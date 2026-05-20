mod constants;
pub(crate) mod db_store;
mod duration;
mod error;
mod path;
mod persistence;
mod store;
mod types;

#[cfg(test)]
mod tests;

// Public API re-exports (preserved for backward compatibility)
pub use constants::{
    GOALS_DIR, GOAL_AGENT_RUNS_DIR, GOAL_ARTIFACTS_DIR, GOAL_BUDGET_CHECKPOINTS_FILE,
    GOAL_DECISIONS_FILE, GOAL_FAILURE_FILE, GOAL_GATE_ARTIFACTS_DIR, GOAL_PRD_FILE,
    GOAL_PROOF_FILE, GOAL_STATE_FILE, GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE,
    GOAL_TEST_SPEC_FILE,
};
pub use error::GoalStateError;
pub(super) use persistence::goals_dir;
pub use store::{FileSystemGoalStateStore, GoalStateStore};
pub use db_store::DbGoalStateStore;

#[cfg(test)]
pub use store::InMemoryGoalStateStore;
pub use types::{
    CreateGoalOptions, GoalArtifact, GoalFailure, GoalPhase, GoalState, GoalStatus,
    GoalTerminalCriteria,
};

// Crate-internal re-exports
pub(crate) use constants::{
    GOAL_AGENT_EXECUTE_TASK_ID, GOAL_AGENT_FOLLOWUPS_RUN_ID, GOAL_AGENT_IMPLEMENT_TASK_ID,
    GOAL_AGENT_PUBLISH_TASK_ID, GOAL_AGENT_TASK_POLICY_FILE, GOAL_AGENT_TASK_PROPOSALS_FILE,
    GOAL_AGENT_TASK_PROPOSAL_MARKER, GOAL_AGENT_VERIFY_TASK_ID, GOAL_AGENT_WORKER_ID,
    GOAL_AGENT_WORKER_ROLE, GOAL_CONTROLLER_ACTOR, GOAL_LOCAL_VERIFY_TASK_ID,
    GOAL_REVIEW_ARTIFACTS_DIR, GOAL_REVIEW_FILE, GOAL_REVIEW_TASK_ID, GOAL_SECURITY_REVIEW_FILE,
    GOAL_SECURITY_REVIEW_TASK_ID,
};
pub(crate) use duration::{
    default_goal_agent_task_budget_secs, format_goal_duration_secs, goal_agent_task_budget_secs,
    parse_budget_duration, parse_goal_duration_secs,
};
pub(crate) use path::is_safe_goal_agent_path;
pub(crate) use persistence::{generate_goal_id, normalize_goal};
