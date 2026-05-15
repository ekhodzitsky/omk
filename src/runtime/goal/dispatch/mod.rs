mod interrupt;
mod runtime;
mod tasks;

pub(crate) use interrupt::watch_goal_control_interrupt;
pub(crate) use runtime::{
    goal_agent_lease_seconds_override, goal_agent_wire_runtime_available,
    goal_agent_worker_count, goal_agent_worker_name, prepare_goal_agent_workers, stop_wire_worker,
};
pub(crate) use tasks::{
    append_agent_execution_task_events, run_goal_agent_task_wave,
};

// Re-exports from parent module so submodules can use `super::` imports
pub(crate) use super::agent::{
    check_task_path_policy, goal_agent_task_policy_payload, validate_goal_agent_task_proposals,
    GoalAgentDispatchPlan, GoalAgentTaskProposal,
};
pub(crate) use super::budget::{evaluate_task_budget, PerTaskBudgetSnapshot};
pub(crate) use super::evidence::{
    extract_goal_agent_task_proposals, write_goal_agent_mutation_snapshot, GoalAgentRunEvidence,
};
pub(crate) use super::planner::controller_task_summary;
pub(crate) use super::proof::write_json_artifact;
pub(crate) use super::state::{
    GoalState, GoalStatus, GOAL_AGENT_RUNS_DIR, GOAL_AGENT_TASK_POLICY_FILE,
    GOAL_AGENT_TASK_PROPOSALS_FILE, GOAL_AGENT_WORKER_ID, GOAL_AGENT_WORKER_ROLE,
    GOAL_ARTIFACTS_DIR, GOAL_CONTROLLER_ACTOR, GOAL_LOCAL_VERIFY_TASK_ID,
};
pub(crate) use super::task_graph::{GoalTaskGraph, GoalTaskStatus};
