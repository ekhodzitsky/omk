use std::time::Duration;

use tracing::{debug, warn};

use super::error::LlmError;

/// Configuration for retry behaviour.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial delay before the first retry.
    pub base_delay: Duration,
    /// Maximum delay between retries (backoff cap).
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
        }
    }
}

/// Execute an asynchronous operation with exponential-backoff retry.
///
/// `is_retryable` is a predicate that decides whether a given error warrants
/// another attempt.
pub(crate) async fn with_retry<F, Fut, T>(
    operation: F,
    policy: &RetryPolicy,
    retryable: fn(&LlmError) -> bool,
) -> Result<T, LlmError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, LlmError>>,
{
    let mut attempt = 0u32;

    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if retryable(&err) && attempt < policy.max_retries => {
                attempt += 1;
                let delay = compute_backoff(attempt, policy);
                warn!(
                    error = %err,
                    attempt,
                    max_retries = policy.max_retries,
                    ?delay,
                    "retryable LLM error, backing off"
                );
                tokio::time::sleep(delay).await;
            }
            Err(err) => {
                debug!(error = %err, attempt, "non-retryable LLM error or retries exhausted");
                return Err(err);
            }
        }
    }
}

fn compute_backoff(attempt: u32, policy: &RetryPolicy) -> Duration {
    let exp = policy.base_delay.mul_f64(2f64.powi(attempt as i32));
    if exp > policy.max_delay {
        policy.max_delay
    } else {
        exp
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::super::error::is_retryable;
    use super::*;

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let policy = RetryPolicy::default();
        let counter = Arc::new(AtomicUsize::new(0));

        let result = with_retry(
            || {
                let c = counter.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(42)
                }
            },
            &policy,
            is_retryable,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_third_try() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        };
        let counter = Arc::new(AtomicUsize::new(0));

        let result = with_retry(
            || {
                let c = counter.clone();
                async move {
                    let attempt = c.fetch_add(1, Ordering::SeqCst);
                    if attempt < 2 {
                        Err(LlmError::TransientNetwork("boom".to_string()))
                    } else {
                        Ok(42)
                    }
                }
            },
            &policy,
            is_retryable,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let policy = RetryPolicy::default();
        let counter = Arc::new(AtomicUsize::new(0));

        let result: Result<i32, LlmError> = with_retry(
            || {
                let c = counter.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(LlmError::Authentication)
                }
            },
            &policy,
            is_retryable,
        )
        .await;

        assert!(matches!(result, Err(LlmError::Authentication)));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_retry_exponential_backoff() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
        };

        assert_eq!(compute_backoff(1, &policy), Duration::from_millis(200));
        assert_eq!(compute_backoff(2, &policy), Duration::from_millis(400));
        assert_eq!(compute_backoff(3, &policy), Duration::from_millis(800));
    }

    #[test]
    fn test_retry_backoff_capped() {
        let policy = RetryPolicy {
            max_retries: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
        };

        assert_eq!(compute_backoff(1, &policy), Duration::from_secs(2));
        assert_eq!(compute_backoff(2, &policy), Duration::from_secs(4));
        assert_eq!(compute_backoff(3, &policy), Duration::from_secs(5)); // capped
        assert_eq!(compute_backoff(10, &policy), Duration::from_secs(5)); // capped
    }
}
