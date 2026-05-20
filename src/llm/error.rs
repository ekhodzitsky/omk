use std::time::Duration;

use thiserror::Error;

/// Errors that can occur when interacting with an LLM.
#[derive(Debug, Error, Clone)]
pub enum LlmError {
    #[error("request timed out after {0:?}")]
    Timeout(Duration),

    #[error("rate limited, retry after {0:?}")]
    RateLimited(Duration),

    #[error("transient network error: {0}")]
    TransientNetwork(String),

    #[error("invalid prompt: {0}")]
    InvalidPrompt(String),

    #[error("context length exceeded: {prompt_tokens} > {max_tokens}")]
    ContextLengthExceeded {
        prompt_tokens: usize,
        max_tokens: usize,
    },

    #[error("authentication failed")]
    Authentication,

    #[error("failed to parse LLM response: {reason}")]
    ParseError { raw: String, reason: String },

    #[error("budget exhausted: {used} / {max} tokens")]
    BudgetExhausted { used: usize, max: usize },

    #[error("io error: {0}")]
    Io(String),
}

impl From<std::io::Error> for LlmError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Returns `true` if the error is retryable.
pub(crate) fn is_retryable(error: &LlmError) -> bool {
    matches!(
        error,
        LlmError::Timeout(_) | LlmError::RateLimited(_) | LlmError::TransientNetwork(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let llm_err: LlmError = io_err.into();
        assert!(matches!(llm_err, LlmError::Io(ref msg) if msg.contains("file not found")));
    }

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(&LlmError::Timeout(Duration::from_secs(1))));
        assert!(is_retryable(&LlmError::RateLimited(Duration::from_secs(1))));
        assert!(is_retryable(&LlmError::TransientNetwork(
            "boom".to_string()
        )));
        assert!(!is_retryable(&LlmError::Authentication));
        assert!(!is_retryable(&LlmError::InvalidPrompt("bad".to_string())));
        assert!(!is_retryable(&LlmError::BudgetExhausted {
            used: 1,
            max: 1
        }));
        assert!(!is_retryable(&LlmError::ParseError {
            raw: "{}".to_string(),
            reason: "test".to_string(),
        }));
    }
}
