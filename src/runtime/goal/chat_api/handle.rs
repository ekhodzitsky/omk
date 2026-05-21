use crate::runtime::goal::GoalMergePolicy;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct CreateChildRequest {
    pub session_id: String,
    pub parent_conv_id: String,
    pub prompt: String,
    pub config: ChildGoalConfig,
}

#[derive(Debug)]
pub struct ChildGoalConfig {
    pub merge_policy: GoalMergePolicy,
    pub enforce_protection: bool,
    pub wire_pool_size: u32,
    pub max_budget_usd: Option<f32>,
}

impl Default for ChildGoalConfig {
    fn default() -> Self {
        Self {
            merge_policy: GoalMergePolicy::Disabled,
            enforce_protection: false,
            wire_pool_size: 3,
            max_budget_usd: None,
        }
    }
}

#[derive(Debug)]
pub struct ChildGoalHandle {
    pub goal_id: String,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
}
