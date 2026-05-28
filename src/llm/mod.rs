//! LLM client and planner module.
//!
//! Provides typed interfaces for calling LLMs, parsing structured output,
//! estimating costs, and planning engineering goals.
//!
//! ## Public API
//!
//! - [`LlmClient`] — trait for LLM completion.
//! - [`MockLlmClient`] — in-memory test implementation.
//! - [`WireLlmClient`] — production implementation over the Wire protocol.
//! - [`Planner`] — trait for goal classification, decomposition, and estimation.
//! - [`LlmPlanner`] — LLM-backed planner implementation.
//! - [`MockPlanner`] — configurable test planner.
//! - [`CostEstimator`] — token counting and USD estimation.
//! - [`TokenBudget`] — tracks token consumption against a cap.
//! - [`RetryPolicy`] — exponential-backoff configuration.
//! - [`LlmError`] — structured error enum.
//!
//! ## Module boundaries
//!
//! This module **does not** depend on `runtime::goal`, `cli::goal`, task graphs,
//! proof semantics, or worktree logic.  It is a pure LLM-call abstraction.

pub mod client;
pub mod cost;
pub mod error;
pub mod planner;
pub mod types;

mod parser;
mod prompt;
mod retry;

pub use client::{LlmClient, LlmClientConfig, WireLlmClient};
#[cfg(test)]
pub use client::MockLlmClient;
pub use cost::CostEstimator;
pub use error::LlmError;
pub use planner::{LlmPlanner, MockPlanner, Planner};
pub use retry::RetryPolicy;
pub use types::{
    Complexity, Difficulty, GoalClassification, GoalKind, LlmResponse, LlmUsage, Plan, RepoContext,
    Slice, TokenBudget,
};
