#[cfg(test)]
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::debug;

use crate::wire::client::WireClient as ExistingWireClient;
use crate::wire::client::WireMessage;
use crate::wire::protocol::{Event, PromptResult};

use super::cost::CostEstimator;
use super::error::is_retryable;
use super::error::LlmError;
use super::prompt;
use super::retry::{with_retry, RetryPolicy};
use super::types::{LlmResponse, TokenBudget};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for an LLM client.
#[derive(Debug, Clone)]
pub struct LlmClientConfig {
    /// Model identifier (e.g. "gpt-4o").
    pub model: String,
    /// Maximum tokens allowed in a single request.
    pub max_tokens: usize,
    /// Sampling temperature (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
    /// Request timeout.
    pub timeout: Duration,
    /// Retry policy for transient failures.
    pub retry_policy: RetryPolicy,
}

impl Default for LlmClientConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o".to_string(),
            max_tokens: 4096,
            temperature: 0.2,
            timeout: Duration::from_secs(60),
            retry_policy: RetryPolicy::default(),
        }
    }
}

// ============================================================================
// Trait
// ============================================================================

/// Abstract interface for LLM completion.
pub trait LlmClient: Send + Sync {
    /// Send a prompt and return the LLM's text response.
    fn complete(
        &self,
        prompt: &str,
        budget: &TokenBudget,
    ) -> impl std::future::Future<Output = Result<LlmResponse, LlmError>> + Send;

    /// Send a prompt with a system message.
    fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
        budget: &TokenBudget,
    ) -> impl std::future::Future<Output = Result<LlmResponse, LlmError>> + Send;
}

// ============================================================================
// Mock implementation
// ============================================================================

/// In-memory mock client for unit tests.
///
/// Pre-configured responses are returned in FIFO order.  All prompts sent
/// through the client can be inspected later via [`MockLlmClient::take_calls`].
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct MockLlmClient {
    responses: Arc<Mutex<VecDeque<String>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl MockLlmClient {
    /// Create a mock with a queue of canned responses.
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Push an additional response onto the queue.
    pub async fn push_response(&self, response: String) {
        self.responses.lock().await.push_back(response);
    }

    /// Return all prompts that have been sent to this client.
    pub async fn take_calls(&self) -> Vec<String> {
        self.calls.lock().await.clone()
    }

    /// Return the number of remaining queued responses.
    pub async fn remaining_responses(&self) -> usize {
        self.responses.lock().await.len()
    }
}

#[cfg(test)]
#[allow(async_fn_in_trait)]
impl LlmClient for MockLlmClient {
    async fn complete(&self, prompt: &str, _budget: &TokenBudget) -> Result<LlmResponse, LlmError> {
        self.calls.lock().await.push(prompt.to_string());
        let content = self.responses.lock().await.pop_front().ok_or_else(|| {
            LlmError::TransientNetwork("mock response queue exhausted".to_string())
        })?;
        Ok(LlmResponse {
            content,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            model: "mock".to_string(),
            finish_reason: "stop".to_string(),
        })
    }

    async fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
        budget: &TokenBudget,
    ) -> Result<LlmResponse, LlmError> {
        self.complete(&prompt::system_prompt(system, prompt), budget)
            .await
    }
}

// ============================================================================
// Wire adapter
// ============================================================================

/// Adapter that wraps the existing Wire client to implement [`LlmClient`].
///
/// This adapter reads the Wire event stream, collecting `ContentPart` text
/// until the prompt response is received.
#[derive(Debug)]
pub struct WireLlmClient<W> {
    wire: Arc<Mutex<W>>,
    config: LlmClientConfig,
    cost_estimator: CostEstimator,
}

impl<W> WireLlmClient<W>
where
    W: ExistingWireClient + Send,
{
    /// Create a new wire-backed LLM client.
    pub fn new(
        wire: Arc<Mutex<W>>,
        config: LlmClientConfig,
        cost_estimator: CostEstimator,
    ) -> Self {
        Self {
            wire,
            config,
            cost_estimator,
        }
    }

    /// Count tokens and enforce both the caller's budget and the model's context limit.
    fn check_budget(&self, text: &str, budget: &TokenBudget) -> Result<usize, LlmError> {
        let tokens = self.cost_estimator.count_tokens(text, &self.config.model)?;
        if tokens > self.config.max_tokens {
            return Err(LlmError::ContextLengthExceeded {
                prompt_tokens: tokens,
                max_tokens: self.config.max_tokens,
            });
        }
        if !budget.can_afford(tokens) {
            return Err(LlmError::BudgetExhausted {
                used: budget.used_tokens(),
                max: budget.max_tokens(),
            });
        }
        Ok(tokens)
    }

    /// Core loop: send a prompt via Wire and collect the response text.
    async fn complete_inner(
        &self,
        prompt: &str,
        budget: &TokenBudget,
    ) -> Result<LlmResponse, LlmError> {
        let prompt_tokens = self.check_budget(prompt, budget)?;

        let id = {
            let mut wire = self.wire.lock().await;
            wire.start_prompt(prompt)
                .await
                .map_err(|e| LlmError::TransientNetwork(e.to_string()))?
        };
        tracing::trace!(request_id = %id, "wire prompt started");

        let mut content = String::new();
        let mut status_tokens: Option<u64> = None;

        loop {
            let msg = match tokio::time::timeout(
                self.config.timeout,
                async {
                    let mut wire = self.wire.lock().await;
                    wire.read_message().await
                },
            )
            .await
            {
                Ok(Ok(m)) => m,
                Ok(Err(e)) => return Err(LlmError::TransientNetwork(e.to_string())),
                Err(_) => {
                    return Err(LlmError::Timeout(self.config.timeout));
                }
            };

            match msg {
                WireMessage::SuccessResponse(resp) if resp.id == id => {
                    let raw = serde_json::to_string(&resp.result).unwrap_or_default();
                    let result: PromptResult =
                        serde_json::from_value(resp.result).map_err(|e| {
                            LlmError::ParseError {
                                raw,
                                reason: e.to_string(),
                            }
                        })?;
                    debug!(status = %result.status, "wire prompt completed");
                    break;
                }
                WireMessage::ErrorResponse(resp) if resp.id == id => {
                    return Err(LlmError::TransientNetwork(format!(
                        "wire error {}: {}",
                        resp.error.code, resp.error.message
                    )));
                }
                WireMessage::Event(ev) => {
                    if let Ok(event) = ev.params.to_event() {
                        match event {
                            Event::ContentPart { text, chunk } => {
                                if let Some(t) = text {
                                    content.push_str(&t);
                                }
                                if let Some(c) = chunk {
                                    content.push_str(&c);
                                }
                            }
                            Event::StatusUpdate {
                                token_usage: Some(tu),
                                ..
                            } => {
                                status_tokens = Some(tu);
                            }
                            Event::TurnEnd => {
                                tracing::trace!("wire turn end");
                            }
                            _ => {}
                        }
                    } else {
                        tracing::trace!(params = ?ev.params, "ignoring malformed wire event");
                    }
                }
                other => {
                    tracing::trace!(?other, "ignoring unrelated wire message");
                }
            }
        }

        let completion_tokens = if let Some(tu) = status_tokens {
            tu as usize
        } else {
            self.cost_estimator
                .count_tokens(&content, &self.config.model)?
        };

        let total_tokens = prompt_tokens + completion_tokens;

        Ok(LlmResponse {
            content,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            model: self.config.model.clone(),
            finish_reason: "stop".to_string(),
        })
    }
}

#[allow(async_fn_in_trait)]
impl<W> LlmClient for WireLlmClient<W>
where
    W: ExistingWireClient + Send,
{
    async fn complete(&self, prompt: &str, budget: &TokenBudget) -> Result<LlmResponse, LlmError> {
        with_retry(
            || self.complete_inner(prompt, budget),
            &self.config.retry_policy,
            is_retryable,
        )
        .await
    }

    async fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
        budget: &TokenBudget,
    ) -> Result<LlmResponse, LlmError> {
        self.complete(&prompt::system_prompt(system, prompt), budget)
            .await
    }
}

#[cfg(test)]
mod tests;
