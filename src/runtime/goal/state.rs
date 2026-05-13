use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

pub const GOALS_DIR: &str = "goals";
pub const GOAL_STATE_FILE: &str = "goal.json";
pub const GOAL_FAILURE_FILE: &str = "failure.json";
pub const GOAL_PRD_FILE: &str = "prd.md";
pub const GOAL_TECHNICAL_PLAN_FILE: &str = "technical-plan.md";
pub const GOAL_TEST_SPEC_FILE: &str = "test-spec.md";
pub const GOAL_TASK_GRAPH_FILE: &str = "task-graph.json";
pub const GOAL_DECISIONS_FILE: &str = "decisions.jsonl";
pub const GOAL_PROOF_FILE: &str = "proof.json";
pub const GOAL_BUDGET_CHECKPOINTS_FILE: &str = "budget-checkpoints.jsonl";
pub const GOAL_ARTIFACTS_DIR: &str = "artifacts";
pub const GOAL_GATE_ARTIFACTS_DIR: &str = "gates";
pub const GOAL_AGENT_RUNS_DIR: &str = "agent-runs";
pub(crate) const GOAL_CONTROLLER_ACTOR: &str = "goal-controller";
pub(crate) const GOAL_LOCAL_VERIFY_TASK_ID: &str = "goal-local-verify";
pub(crate) const GOAL_AGENT_EXECUTE_TASK_ID: &str = "goal-agent-execute";
pub(crate) const GOAL_AGENT_IMPLEMENT_TASK_ID: &str = "goal-agent-implement";
pub(crate) const GOAL_AGENT_VERIFY_TASK_ID: &str = "goal-agent-verify";
pub(crate) const GOAL_AGENT_PUBLISH_TASK_ID: &str = "goal-agent-publish-crates-io";
pub(crate) const GOAL_AGENT_FOLLOWUPS_RUN_ID: &str = "goal-agent-followups";
pub(crate) const GOAL_AGENT_TASK_POLICY_FILE: &str = "task-policy.json";
pub(crate) const GOAL_AGENT_TASK_PROPOSALS_FILE: &str = "agent-task-proposals.json";
pub(crate) const GOAL_AGENT_TASK_PROPOSAL_MARKER: &str = "OMK_TASK_PROPOSAL:";
pub(crate) const GOAL_REVIEW_TASK_ID: &str = "goal-review";
pub(crate) const GOAL_SECURITY_REVIEW_TASK_ID: &str = "goal-security-review";
pub(crate) const GOAL_AGENT_WORKER_ID: &str = "goal-agent-worker-0";
pub(crate) const GOAL_AGENT_WORKER_ROLE: &str = "executor";
pub(crate) const GOAL_REVIEW_ARTIFACTS_DIR: &str = "reviews";
pub(crate) const GOAL_REVIEW_FILE: &str = "goal-review.md";
pub(crate) const GOAL_SECURITY_REVIEW_FILE: &str = "goal-security-review.md";

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
}

impl GoalState {
    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join(GOAL_STATE_FILE)
    }

    pub async fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&self.state_file(), json.as_bytes()).await
    }

    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_STATE_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal state: {}", path.display()))?;
        let mut state: Self = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal state: {}", path.display()))?;
        state.state_dir = goal_dir.to_path_buf();
        Ok(state)
    }
}

pub fn goals_dir() -> PathBuf {
    crate::runtime::config::omk_state_dir().join(GOALS_DIR)
}

pub(crate) fn generate_goal_id() -> String {
    let suffix = Uuid::new_v4().to_string();
    format!(
        "goal-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S-%3f"),
        &suffix[..8]
    )
}

pub(crate) fn normalize_goal(goal: &str) -> String {
    goal.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn is_safe_goal_agent_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "project files" {
        return true;
    }
    let path = Path::new(trimmed);
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && trimmed != ".git"
        && !trimmed.starts_with(".git/")
}

pub(crate) fn default_goal_agent_task_budget_secs() -> u64 {
    300
}

pub(crate) fn goal_agent_task_budget_secs(state: &GoalState, requested_secs: u64) -> u64 {
    let Some(total_budget_secs) = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs)
    else {
        return requested_secs;
    };
    let per_task_ceiling = if total_budget_secs < 60 {
        total_budget_secs.max(1)
    } else {
        (total_budget_secs / 4).max(60)
    };
    requested_secs.min(per_task_ceiling)
}

pub(crate) fn parse_goal_duration_secs(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (number, multiplier) = match trimmed.chars().last()? {
        's' | 'S' => (&trimmed[..trimmed.len() - 1], 1),
        'm' | 'M' => (&trimmed[..trimmed.len() - 1], 60),
        'h' | 'H' => (&trimmed[..trimmed.len() - 1], 60 * 60),
        'd' | 'D' => (&trimmed[..trimmed.len() - 1], 24 * 60 * 60),
        _ => (trimmed, 1),
    };
    number.trim().parse::<u64>().ok()?.checked_mul(multiplier)
}

pub(crate) fn format_goal_duration_secs(secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    if secs != 0 && secs % DAY == 0 {
        format!("{}d", secs / DAY)
    } else if secs != 0 && secs % HOUR == 0 {
        format!("{}h", secs / HOUR)
    } else if secs != 0 && secs % MINUTE == 0 {
        format!("{}m", secs / MINUTE)
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn goal_status_serializes_as_snake_case() {
        let value = serde_json::to_value(GoalStatus::NotReady).unwrap();
        assert_eq!(value, "not_ready");
    }

    #[test]
    fn paused_goal_status_serializes_as_snake_case() {
        let value = serde_json::to_value(GoalStatus::Paused).unwrap();
        assert_eq!(value, "paused");
    }

    #[test]
    fn goal_phase_serializes_as_snake_case() {
        let value = serde_json::to_value(GoalPhase::VerificationDesign).unwrap();
        assert_eq!(value, "verification_design");
    }

    #[test]
    fn normalize_goal_collapses_whitespace() {
        assert_eq!(normalize_goal("  ship   it\nwell  "), "ship it well");
    }

    #[test]
    fn goal_duration_formats_to_stable_compact_units() {
        assert_eq!(format_goal_duration_secs(0), "0s");
        assert_eq!(format_goal_duration_secs(59), "59s");
        assert_eq!(format_goal_duration_secs(60), "1m");
        assert_eq!(format_goal_duration_secs(3_600), "1h");
        assert_eq!(format_goal_duration_secs(86_400), "1d");
    }

    #[tokio::test]
    async fn goal_state_loads_legacy_json_with_safe_defaults() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(GOAL_STATE_FILE),
            r#"{
              "goal_id": "goal-legacy",
              "original_goal": "Ship safely",
              "normalized_goal": "Ship safely",
              "status": "not_ready",
              "created_at": "2026-05-13T00:00:00Z",
              "updated_at": "2026-05-13T00:00:01Z"
            }"#,
        )
        .unwrap();

        let state = GoalState::load(temp.path()).await.unwrap();

        assert_eq!(state.version, 1);
        assert_eq!(state.phase, GoalPhase::Intake);
        assert!(!state.until_ready);
        assert!(state.terminal_criteria.proof_required);
        assert!(state.terminal_criteria.gates_required);
        assert!(state.terminal_criteria.human_blockers_stop);
        assert!(state.artifacts.is_empty());
        assert_eq!(state.state_dir, temp.path());
    }

    #[tokio::test]
    async fn goal_state_load_rehomes_stale_persisted_state_dir() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(GOAL_STATE_FILE),
            r#"{
              "version": 1,
              "goal_id": "goal-moved",
              "original_goal": "Resume after move",
              "normalized_goal": "Resume after move",
              "status": "paused",
              "phase": "proof",
              "created_at": "2026-05-13T00:00:00Z",
              "updated_at": "2026-05-13T00:00:01Z",
              "until_ready": true,
              "terminal_criteria": {
                "proof_required": true,
                "gates_required": true,
                "human_blockers_stop": true
              },
              "state_dir": "/old/machine/.local/state/omk/goals/goal-moved"
            }"#,
        )
        .unwrap();

        let state = GoalState::load(temp.path()).await.unwrap();

        assert_eq!(state.goal_id, "goal-moved");
        assert_eq!(state.status, GoalStatus::Paused);
        assert_eq!(state.state_dir, temp.path());
    }
}
