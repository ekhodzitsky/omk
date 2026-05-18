use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalDeliverySlicePlan {
    pub goal_id: String,
    pub slices: Vec<GoalDeliverySlice>,
    pub overlap_serializations: Vec<GoalDeliveryOverlapSerialization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalDeliverySlice {
    pub slice_id: String,
    pub task_id: String,
    pub owner_role: String,
    pub read_scope: Vec<String>,
    pub write_scope: Vec<String>,
    pub dependencies: Vec<String>,
    pub branch_name: String,
    pub worktree_name: String,
    pub worktree_path: PathBuf,
    pub gates: Vec<String>,
    pub review_needs: Vec<String>,
    pub pr_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalDeliveryOverlapSerialization {
    pub blocked_slice_id: String,
    pub serializes_after: String,
    pub kind: String,
    pub path: String,
}

pub(super) struct AccessOverlap {
    pub(super) kind: &'static str,
    pub(super) path: String,
}
