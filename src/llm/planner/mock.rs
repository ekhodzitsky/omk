use super::{Complexity, GoalClassification, LlmError, Plan, Planner, RepoContext};
use async_trait::async_trait;

/// A mock planner that returns pre-configured results.
#[derive(Debug, Clone)]
pub struct MockPlanner {
    classification: Option<GoalClassification>,
    plan: Option<Plan>,
    criteria: Option<Vec<String>>,
    complexity: Option<Complexity>,
}

impl MockPlanner {
    /// Create an empty mock.  All methods will return an error until configured.
    pub fn new() -> Self {
        Self {
            classification: None,
            plan: None,
            criteria: None,
            complexity: None,
        }
    }

    /// Set the canned classification result.
    pub fn with_classification(mut self, c: GoalClassification) -> Self {
        self.classification = Some(c);
        self
    }

    /// Set the canned plan result.
    pub fn with_plan(mut self, p: Plan) -> Self {
        self.plan = Some(p);
        self
    }

    /// Set the canned criteria result.
    pub fn with_criteria(mut self, c: Vec<String>) -> Self {
        self.criteria = Some(c);
        self
    }

    /// Set the canned complexity result.
    pub fn with_complexity(mut self, c: Complexity) -> Self {
        self.complexity = Some(c);
        self
    }
}

impl Default for MockPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Planner for MockPlanner {
    async fn classify(&self, _goal_text: &str) -> Result<GoalClassification, LlmError> {
        self.classification
            .clone()
            .ok_or_else(|| LlmError::InvalidPrompt("mock classification not set".to_string()))
    }

    async fn decompose(&self, _goal_text: &str, _context: &RepoContext) -> Result<Plan, LlmError> {
        self.plan
            .clone()
            .ok_or_else(|| LlmError::InvalidPrompt("mock plan not set".to_string()))
    }

    async fn generate_criteria(
        &self,
        _goal_text: &str,
        _plan: &Plan,
    ) -> Result<Vec<String>, LlmError> {
        self.criteria
            .clone()
            .ok_or_else(|| LlmError::InvalidPrompt("mock criteria not set".to_string()))
    }

    async fn estimate_complexity(
        &self,
        _goal_text: &str,
        _plan: &Plan,
    ) -> Result<Complexity, LlmError> {
        self.complexity
            .clone()
            .ok_or_else(|| LlmError::InvalidPrompt("mock complexity not set".to_string()))
    }
}
