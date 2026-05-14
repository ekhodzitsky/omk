mod metadata;
mod persist;
mod worktree;

pub(crate) use persist::{load_task_delivery_metadata, preserve_delivery_metadata_in_value};
pub(crate) use worktree::{ensure_worktree_delivery_targets, record_worktree_delivery_metadata};

pub use metadata::{
    GoalTaskDeliveryMetadata, GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryRecord,
    GoalTaskDeliveryStatus,
};
pub use persist::{
    load_goal_task_delivery_records, read_goal_task_delivery_metadata,
    update_goal_task_delivery_metadata,
};
