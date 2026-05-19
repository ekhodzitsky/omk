use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GoalId(String);

impl GoalId {
    pub fn generate() -> Self {
        Self(super::state::generate_goal_id())
    }

    pub fn parse(value: &str) -> Result<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            anyhow::bail!("goal id cannot be empty");
        }
        if !trimmed.starts_with("goal-") {
            anyhow::bail!("goal id must start with `goal-`");
        }
        if trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
            anyhow::bail!("goal id must be a safe path component");
        }
        if trimmed.chars().any(char::is_control) {
            anyhow::bail!("goal id contains control characters");
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for GoalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for GoalId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoalBudget {
    pub time: Option<String>,
    pub tokens: Option<u64>,
    pub usd: Option<f64>,
    pub max_agents: Option<usize>,
}

impl GoalBudget {
    pub fn new(
        time: Option<String>,
        tokens: Option<u64>,
        usd: Option<f64>,
        max_agents: Option<usize>,
    ) -> Result<Self> {
        if let Some(time) = time.as_deref() {
            super::state::parse_budget_duration(time)
                .with_context(|| format!("invalid goal budget time: {time}"))?;
        }
        if tokens == Some(0) {
            anyhow::bail!("goal token budget must be greater than zero");
        }
        if let Some(usd) = usd {
            if !usd.is_finite() || usd <= 0.0 {
                anyhow::bail!("goal USD budget must be a positive, finite number");
            }
        }
        if max_agents == Some(0) {
            anyhow::bail!("goal max agents must be greater than zero");
        }
        Ok(Self {
            time,
            tokens,
            usd,
            max_agents,
        })
    }

    pub(crate) fn from_options(options: super::state::CreateGoalOptions) -> Result<Self> {
        Self::new(
            options.budget_time,
            options.budget_tokens,
            options.budget_usd,
            options.max_agents,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalControllerStepKind {
    Plan,
    Verify,
    Execute,
    Review,
    Deliver,
    Blocked,
}

impl GoalControllerStepKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Verify => "verify",
            Self::Execute => "execute",
            Self::Review => "review",
            Self::Deliver => "deliver",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct GoalControllerStep {
    pub kind: GoalControllerStepKind,
    pub(crate) status: super::state::GoalStatus,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub(crate) struct GoalRunUntilReadyOutcome {
    pub(crate) state: super::state::GoalState,
    pub(crate) proof: super::proof::GoalProof,
    pub(crate) steps: Vec<GoalControllerStep>,
    pub(crate) blocker: Option<String>,
    pub(crate) policy_evidence_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_id_parse_accepts_valid_id() {
        let id = GoalId::parse("goal-20240101-120000-001-abcdef").unwrap();
        assert_eq!(id.as_str(), "goal-20240101-120000-001-abcdef");
    }

    #[test]
    fn goal_id_parse_rejects_empty() {
        assert!(GoalId::parse("").is_err());
    }

    #[test]
    fn goal_id_parse_rejects_missing_prefix() {
        assert!(GoalId::parse("not-a-goal").is_err());
    }

    #[test]
    fn goal_id_parse_rejects_traversal() {
        assert!(GoalId::parse("goal-../etc").is_err());
        assert!(GoalId::parse("goal-foo/bar").is_err());
    }

    #[test]
    fn goal_id_parse_rejects_control_chars() {
        assert!(GoalId::parse("goal-\x01").is_err());
    }

    #[test]
    fn goal_id_display() {
        let id = GoalId::parse("goal-123").unwrap();
        assert_eq!(id.to_string(), "goal-123");
    }

    #[test]
    fn goal_id_from_str() {
        let id: GoalId = "goal-abc".parse().unwrap();
        assert_eq!(id.as_str(), "goal-abc");
    }

    #[test]
    fn goal_budget_new_accepts_valid() {
        let b = GoalBudget::new(Some("1h".to_string()), Some(100), Some(10.0), Some(5)).unwrap();
        assert_eq!(b.tokens, Some(100));
        assert_eq!(b.usd, Some(10.0));
    }

    #[test]
    fn goal_budget_new_rejects_zero_tokens() {
        assert!(GoalBudget::new(None, Some(0), None, None).is_err());
    }

    #[test]
    fn goal_budget_new_rejects_non_finite_usd() {
        assert!(GoalBudget::new(None, None, Some(f64::NAN), None).is_err());
        assert!(GoalBudget::new(None, None, Some(f64::INFINITY), None).is_err());
    }

    #[test]
    fn goal_budget_new_rejects_zero_max_agents() {
        assert!(GoalBudget::new(None, None, None, Some(0)).is_err());
    }

    #[test]
    fn goal_controller_step_kind_as_str() {
        assert_eq!(GoalControllerStepKind::Plan.as_str(), "plan");
        assert_eq!(GoalControllerStepKind::Verify.as_str(), "verify");
        assert_eq!(GoalControllerStepKind::Execute.as_str(), "execute");
        assert_eq!(GoalControllerStepKind::Review.as_str(), "review");
        assert_eq!(GoalControllerStepKind::Deliver.as_str(), "deliver");
        assert_eq!(GoalControllerStepKind::Blocked.as_str(), "blocked");
    }
}
