//! Retry logic with exponential backoff for async operations.

use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Retry configuration.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Retry an async operation until it succeeds or max attempts are reached.
///
/// # Example
/// ```
/// use omk::runtime::retry::{retry, RetryConfig};
///
/// # async fn example() {
/// let result: Result<String, std::io::Error> = retry(RetryConfig::default(), || async {
///     tokio::fs::read_to_string("config.toml").await
/// }).await;
/// # }
/// ```
pub async fn retry<F, Fut, T, E>(config: RetryConfig, mut op: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 1;
    let mut delay_ms = config.base_delay_ms;

    loop {
        match op().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt >= config.max_attempts {
                    warn!(attempt = attempt, error = %e, "All retry attempts exhausted");
                    return Err(e);
                }
                warn!(attempt = attempt, error = %e, delay_ms = delay_ms, "Retrying after error");
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                delay_ms = ((delay_ms as f64 * config.backoff_multiplier) as u64)
                    .min(config.max_delay_ms);
                attempt += 1;
            }
        }
    }
}

/// Retry a fallible CLI command, returning its output string.
pub async fn retry_command(
    config: RetryConfig,
    cmd: &mut tokio::process::Command,
) -> anyhow::Result<String> {
    let mut attempt = 1;
    let mut delay_ms = config.base_delay_ms;

    loop {
        let output = cmd.output().await?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if attempt >= config.max_attempts {
            anyhow::bail!(
                "Command failed after {} attempts. stderr: {}",
                attempt,
                stderr
            );
        }

        warn!(
            attempt = attempt,
            stderr = %stderr,
            delay_ms = delay_ms,
            "Command failed, retrying"
        );
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        delay_ms = ((delay_ms as f64 * config.backoff_multiplier) as u64).min(config.max_delay_ms);
        attempt += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_succeeds_first_try() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay_ms: 10,
            ..Default::default()
        };
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();

        let result = retry(config, || async {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok::<_, std::io::Error>(42)
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_eventually_succeeds() {
        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 10,
            ..Default::default()
        };
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();

        let result = retry(config, || async {
            let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < 2 {
                Err::<i32, std::io::Error>(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "not yet",
                ))
            } else {
                Ok(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
