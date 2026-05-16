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
