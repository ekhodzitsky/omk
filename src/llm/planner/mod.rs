use std::sync::Arc;

use async_trait::async_trait;
use tracing::debug;

use super::client::LlmClient;
use super::error::LlmError;
use super::parser::{
    parse_classification_json, parse_complexity_json, parse_criteria_json, parse_plan_json,
};
use super::prompt;
use super::types::{Complexity, GoalClassification, Plan, RepoContext, TokenBudget};

// ============================================================================
// Trait
// ============================================================================

/// Plans and classifies engineering goals using an LLM.
#[async_trait]
pub trait Planner: Send + Sync {
    /// Classify a goal into a known kind.
    async fn classify(&self, goal_text: &str) -> Result<GoalClassification, LlmError>;

    /// Decompose a goal into a plan with slices and dependencies.
    async fn decompose(&self, goal_text: &str, context: &RepoContext) -> Result<Plan, LlmError>;

    /// Generate acceptance criteria for a plan.
    async fn generate_criteria(
        &self,
        goal_text: &str,
        plan: &Plan,
    ) -> Result<Vec<String>, LlmError>;

    /// Estimate the complexity of a plan.
    async fn estimate_complexity(
        &self,
        goal_text: &str,
        plan: &Plan,
    ) -> Result<Complexity, LlmError>;
}

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// A planner that delegates to an [`LlmClient`].
#[derive(Debug)]
pub struct LlmPlanner<C: LlmClient> {
    client: Arc<C>,
    budget: TokenBudget,
}

impl<C: LlmClient> LlmPlanner<C> {
    /// Create a new planner with the given client and token budget.
    pub fn new(client: Arc<C>, budget: TokenBudget) -> Self {
        Self { client, budget }
    }
}

#[async_trait]
impl<C: LlmClient> Planner for LlmPlanner<C> {
    async fn classify(&self, goal_text: &str) -> Result<GoalClassification, LlmError> {
        debug!(goal = %goal_text, "classifying goal");
        let prompt = prompt::classification_prompt(goal_text);
        let response = self.client.complete(&prompt, &self.budget).await?;
        parse_classification_json(&response.content)
    }

    async fn decompose(&self, goal_text: &str, context: &RepoContext) -> Result<Plan, LlmError> {
        debug!(goal = %goal_text, "decomposing goal");
        let prompt = prompt::decomposition_prompt(goal_text, context);
        let response = self.client.complete(&prompt, &self.budget).await?;
        parse_plan_json(goal_text, &response.content)
    }

    async fn generate_criteria(
        &self,
        goal_text: &str,
        plan: &Plan,
    ) -> Result<Vec<String>, LlmError> {
        debug!(goal = %goal_text, "generating acceptance criteria");
        let prompt = prompt::criteria_prompt(goal_text, plan);
        let response = self.client.complete(&prompt, &self.budget).await?;
        parse_criteria_json(&response.content)
    }

    async fn estimate_complexity(
        &self,
        goal_text: &str,
        plan: &Plan,
    ) -> Result<Complexity, LlmError> {
        debug!(goal = %goal_text, "estimating complexity");
        let prompt = prompt::complexity_prompt(goal_text, plan);
        let response = self.client.complete(&prompt, &self.budget).await?;
        parse_complexity_json(&response.content)
    }
}

mod mock;
pub use mock::MockPlanner;

#[cfg(test)]
mod tests;
