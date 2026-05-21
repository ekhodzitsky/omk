use std::fmt;

use serde::{Deserialize, Serialize};

use super::cost::CostEstimator;

// ============================================================================
// Budget
// ============================================================================

/// Tracks how many tokens have been consumed against a maximum allowance.
///
/// Invariants: `used_tokens` is always clamped to `max_tokens`.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    max_tokens: usize,
    used_tokens: usize,
}

impl TokenBudget {
    /// Create a new budget.
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
        }
    }

    /// Maximum tokens allowed.
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Tokens already consumed.
    pub fn used_tokens(&self) -> usize {
        self.used_tokens
    }

    /// Remaining tokens in the budget.
    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Returns `true` if the budget can afford the estimated additional tokens.
    pub fn can_afford(&self, estimated_tokens: usize) -> bool {
        self.used_tokens.saturating_add(estimated_tokens) <= self.max_tokens
    }

    /// Record token usage.
    pub fn record_usage(&mut self, tokens: usize) {
        self.used_tokens = self.used_tokens.saturating_add(tokens);
    }
}

// ============================================================================
// LLM Response
// ============================================================================

/// A single completion response from an LLM.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// Generated text content.
    pub content: String,
    /// Tokens in the prompt.
    pub prompt_tokens: usize,
    /// Tokens in the completion.
    pub completion_tokens: usize,
    /// Total tokens consumed (prompt + completion).
    pub total_tokens: usize,
    /// Model identifier used for the request.
    pub model: String,
    /// Reason the generation stopped (e.g. "stop", "length").
    pub finish_reason: String,
}

impl LlmResponse {
    /// Convert this response into an [`LlmUsage`] by estimating cost.
    pub fn to_usage(&self, estimator: &CostEstimator) -> LlmUsage {
        LlmUsage {
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
            estimated_usd: estimator.estimate(
                self.prompt_tokens,
                self.completion_tokens,
                &self.model,
            ),
        }
    }
}

/// Token and cost usage for a completed call.
#[derive(Debug, Clone)]
pub struct LlmUsage {
    /// Tokens in the prompt.
    pub prompt_tokens: usize,
    /// Tokens in the completion.
    pub completion_tokens: usize,
    /// Total tokens consumed.
    pub total_tokens: usize,
    /// Estimated cost in US dollars.
    pub estimated_usd: f64,
}

// ============================================================================
// Repo Context
// ============================================================================

/// Lightweight repository metadata used to ground planning prompts.
#[derive(Debug, Clone)]
pub struct RepoContext {
    /// Primary programming language (e.g. "Rust").
    pub primary_language: Option<String>,
    /// Total number of files in the repository.
    pub file_count: usize,
    /// Top-level file or directory names.
    pub top_level_files: Vec<String>,
    /// Whether the repository has a test suite.
    pub has_tests: bool,
    /// Whether the repository has CI configuration.
    pub has_ci: bool,
}

// ============================================================================
// Goal Classification
// ============================================================================

/// The kind of engineering work a goal represents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalKind {
    Greenfield,
    Rewrite,
    Repair,
    Audit,
    Migration,
    Vague,
}

impl fmt::Display for GoalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoalKind::Greenfield => write!(f, "greenfield"),
            GoalKind::Rewrite => write!(f, "rewrite"),
            GoalKind::Repair => write!(f, "repair"),
            GoalKind::Audit => write!(f, "audit"),
            GoalKind::Migration => write!(f, "migration"),
            GoalKind::Vague => write!(f, "vague"),
        }
    }
}

/// Result of classifying a goal.
#[derive(Debug, Clone)]
pub struct GoalClassification {
    /// Classified kind of engineering work.
    pub kind: GoalKind,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f32,
    /// Explanation for the classification.
    pub reasoning: String,
    /// Whether the goal is testable.
    pub is_testable: bool,
    /// Suggested refinement if the goal is vague.
    pub suggested_refinement: Option<String>,
}

// ============================================================================
// Plan & Slice
// ============================================================================

/// Estimated difficulty of a single slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Trivial,
    Easy,
    Medium,
    Hard,
    Complex,
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Difficulty::Trivial => write!(f, "trivial"),
            Difficulty::Easy => write!(f, "easy"),
            Difficulty::Medium => write!(f, "medium"),
            Difficulty::Hard => write!(f, "hard"),
            Difficulty::Complex => write!(f, "complex"),
        }
    }
}

/// A single unit of work within a plan.
#[derive(Debug, Clone)]
pub struct Slice {
    /// Unique identifier for this slice.
    pub id: String,
    /// Human-readable description of the work.
    pub description: String,
    /// Files expected to be modified.
    pub write_set: Vec<String>,
    /// Estimated difficulty of implementation.
    pub estimated_difficulty: Difficulty,
}

/// Overall complexity estimate for a plan.
#[derive(Debug, Clone)]
pub struct Complexity {
    /// Complexity score from 1 to 10.
    pub score: u8,
    /// Explanation for the score.
    pub reasoning: String,
    /// Estimated hours to complete, if available.
    pub estimated_hours: Option<f32>,
}

/// A decomposed plan for a goal.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Original goal text.
    pub goal_text: String,
    /// Classified kind of the goal.
    pub kind: GoalKind,
    /// Overall complexity estimate.
    pub complexity: Complexity,
    /// Ordered sequence of work slices.
    pub slices: Vec<Slice>,
    /// Dependency edges as `(before, after)` indices into `slices`.
    pub dependencies: Vec<(usize, usize)>,
    /// Verifiable acceptance criteria.
    pub acceptance_criteria: Vec<String>,
    /// Estimated token consumption for the plan.
    pub estimated_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_budget_remaining() {
        let mut budget = TokenBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        budget.record_usage(300);
        assert_eq!(budget.remaining(), 700);
    }

    #[test]
    fn test_token_budget_can_afford() {
        let mut budget = TokenBudget::new(100);
        budget.record_usage(80);
        assert!(budget.can_afford(20));
        assert!(!budget.can_afford(21));
    }

    #[test]
    fn test_llm_response_to_usage() {
        let resp = LlmResponse {
            content: "hello".to_string(),
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            model: "gpt-4".to_string(),
            finish_reason: "stop".to_string(),
        };
        let estimator = CostEstimator::new();
        let usage = resp.to_usage(&estimator);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert!(usage.estimated_usd > 0.0);
    }

    #[test]
    fn test_goal_kind_display() {
        assert_eq!(GoalKind::Greenfield.to_string(), "greenfield");
        assert_eq!(GoalKind::Rewrite.to_string(), "rewrite");
        assert_eq!(GoalKind::Repair.to_string(), "repair");
        assert_eq!(GoalKind::Audit.to_string(), "audit");
        assert_eq!(GoalKind::Migration.to_string(), "migration");
        assert_eq!(GoalKind::Vague.to_string(), "vague");
    }

    #[test]
    fn test_difficulty_display() {
        assert_eq!(Difficulty::Trivial.to_string(), "trivial");
        assert_eq!(Difficulty::Easy.to_string(), "easy");
        assert_eq!(Difficulty::Medium.to_string(), "medium");
        assert_eq!(Difficulty::Hard.to_string(), "hard");
        assert_eq!(Difficulty::Complex.to_string(), "complex");
    }
}
