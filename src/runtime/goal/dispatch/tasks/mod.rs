mod payload;
mod results;
mod scheduler;
mod wave;

#[cfg(test)]
mod tests;

pub use payload::{task_dispatch_accepted_payload, task_dispatch_rejected_payload};
pub use results::{
    read_goal_agent_worker_results, summarize_goal_agent_worker_results,
};
pub use scheduler::goal_agent_scheduler_tasks;

pub(crate) use results::append_agent_execution_task_events;
pub(crate) use wave::run_goal_agent_task_wave;

// Re-exports from parent module so submodules can use `super::` imports
pub(crate) use super::{
    check_task_path_policy, controller_task_summary, evaluate_task_budget,
    extract_goal_agent_task_proposals, goal_agent_lease_seconds_override,
    goal_agent_task_policy_payload, goal_agent_wire_runtime_available, goal_agent_worker_count,
    goal_agent_worker_name, prepare_goal_agent_workers, stop_wire_worker,
    validate_goal_agent_task_proposals, write_goal_agent_mutation_snapshot, write_json_artifact,
    GoalAgentDispatchPlan, GoalAgentRunEvidence, GoalAgentTaskProposal, GoalState,
    PerTaskBudgetSnapshot, GOAL_AGENT_RUNS_DIR, GOAL_AGENT_TASK_POLICY_FILE,
    GOAL_AGENT_TASK_PROPOSALS_FILE, GOAL_AGENT_WORKER_ID, GOAL_AGENT_WORKER_ROLE,
    GOAL_ARTIFACTS_DIR, GOAL_CONTROLLER_ACTOR, GOAL_LOCAL_VERIFY_TASK_ID, GoalTaskGraph,
    GoalTaskStatus, watch_goal_control_interrupt,
};
pub(crate) use crate::runtime::config::{EVENTS_FILE, OUTBOX_FILE, WORKERS_DIR};
pub(crate) use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
pub(crate) use crate::runtime::scheduler::runner::TeamRunner;
pub(crate) use crate::runtime::scheduler::task::Task;
pub(crate) use crate::runtime::wire_worker::WireWorkerAdapter;
pub(crate) use crate::runtime::worker::WorkerSpec;
