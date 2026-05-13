mod delivery;
mod model;
mod mutation;

pub(crate) use delivery::{load_task_delivery_metadata, preserve_delivery_metadata_in_value};
pub use model::{GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};
pub use mutation::GoalTaskGraphSummary;
pub(crate) use mutation::{
    apply_agent_execution_task_result, apply_agent_followup_task_results,
    apply_agent_proposed_task_mutations, goal_task_done, pending_goal_agent_followup_proposals,
    summarize_task_graph,
};
