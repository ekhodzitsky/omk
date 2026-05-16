use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Running,
    Ready,
    NotReady,
    BlockedOnHuman,
    BlockedOnExternal,
    NeedsMoreBudget,
    FailedInfra,
    Paused,
    Cancelled,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalStatus::Running => "running",
            GoalStatus::Ready => "ready",
            GoalStatus::NotReady => "not_ready",
            GoalStatus::BlockedOnHuman => "blocked_on_human",
            GoalStatus::BlockedOnExternal => "blocked_on_external",
            GoalStatus::NeedsMoreBudget => "needs_more_budget",
            GoalStatus::FailedInfra => "failed_infra",
            GoalStatus::Paused => "paused",
            GoalStatus::Cancelled => "cancelled",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalPhase {
    Intake,
    Planning,
    Decomposition,
    Execution,
    VerificationDesign,
    Proof,
}

impl std::fmt::Display for GoalPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalPhase::Intake => "intake",
            GoalPhase::Planning => "planning",
            GoalPhase::Decomposition => "decomposition",
            GoalPhase::Execution => "execution",
            GoalPhase::VerificationDesign => "verification_design",
            GoalPhase::Proof => "proof",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTerminalCriteria {
    pub proof_required: bool,
    pub gates_required: bool,
    pub human_blockers_stop: bool,
}

impl Default for GoalTerminalCriteria {
    fn default() -> Self {
        Self {
            proof_required: true,
            gates_required: true,
            human_blockers_stop: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalFailure {
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalArtifact {
    pub kind: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalState {
    #[serde(default = "default_goal_version")]
    pub version: u32,
    pub goal_id: String,
    pub original_goal: String,
    pub normalized_goal: String,
    pub status: GoalStatus,
    #[serde(default = "default_goal_phase")]
    pub phase: GoalPhase,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub until_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_agents: Option<usize>,
    #[serde(default)]
    pub terminal_criteria: GoalTerminalCriteria,
    #[serde(default)]
    pub artifacts: Vec<GoalArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<GoalFailure>,
    #[serde(default)]
    pub state_dir: PathBuf,
}

fn default_goal_version() -> u32 {
    1
}

fn default_goal_phase() -> GoalPhase {
    GoalPhase::Intake
}

#[derive(Debug, Clone)]
pub struct CreateGoalOptions {
    pub until_ready: bool,
    pub budget_time: Option<String>,
    pub budget_tokens: Option<u64>,
    pub budget_usd: Option<f64>,
    pub max_agents: Option<usize>,
    pub delivery_policy: super::super::GoalDeliveryPolicy,
}

impl Default for CreateGoalOptions {
    fn default() -> Self {
        Self {
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            delivery_policy: super::super::GoalDeliveryPolicy::Local,
        }
    }
}
