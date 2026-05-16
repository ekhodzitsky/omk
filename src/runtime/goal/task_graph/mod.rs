mod delivery;
mod model;
mod mutation;

pub use delivery::{
    all_slices_done, load_goal_task_delivery_records, plan_goal_delivery_slices,
    read_goal_task_delivery_metadata, ready_delivery_slices, record_goal_delivery_slice_plan,
    update_goal_task_delivery_metadata, GoalDeliveryOverlapSerialization, GoalDeliverySlice,
    GoalDeliverySlicePlan, GoalTaskDeliveryMetadata, GoalTaskDeliveryMetadataUpdate,
    GoalTaskDeliveryRecord, GoalTaskDeliveryStatus,
};
pub(crate) use delivery::{
    ensure_worktree_delivery_targets, load_task_delivery_metadata,
    preserve_delivery_metadata_in_value, record_worktree_delivery_metadata,
};
pub use model::{GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};
pub use mutation::GoalTaskGraphSummary;
pub(crate) use mutation::{
    apply_agent_execution_task_result, apply_agent_followup_task_results,
    apply_agent_proposed_task_mutations, apply_agent_task_result_by_id, goal_agent_execution_done,
    goal_task_done, pending_goal_agent_followup_proposals, spawn_cleanup_task,
    summarize_task_graph,
};
