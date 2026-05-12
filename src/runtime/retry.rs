//! Retry logic with exponential backoff for async operations.

use std::future::Future;
use std::time::Duration;
use tracing::warn;

/// Retry configuration.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub rate_limit_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
            rate_limit_delay_ms: 30_000,
        }
    }
}

/// Check if stderr indicates a rate-limit response.
pub fn is_rate_limited(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("429") || lower.contains("rate limit") || lower.contains("too many requests")
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
#[allow(dead_code)]
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
                delay_ms =
                    ((delay_ms as f64 * config.backoff_multiplier) as u64).min(config.max_delay_ms);
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
        let output = tokio::time::timeout(Duration::from_secs(60), cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("Command timed out after 60s"))??;
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

        if is_rate_limited(&stderr) {
            warn!(
                attempt = attempt,
                stderr = %stderr,
                delay_ms = config.rate_limit_delay_ms,
                "Rate limit detected, retrying with longer backoff"
            );
            tokio::time::sleep(Duration::from_millis(config.rate_limit_delay_ms)).await;
        } else {
            warn!(
                attempt = attempt,
                stderr = %stderr,
                delay_ms = delay_ms,
                "Command failed, retrying"
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms =
                ((delay_ms as f64 * config.backoff_multiplier) as u64).min(config.max_delay_ms);
        }
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
                Err::<i32, std::io::Error>(std::io::Error::other("not yet"))
            } else {
                Ok(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn test_is_rate_limited_429() {
        assert!(is_rate_limited("HTTP 429 Too Many Requests"));
    }

    #[test]
    fn test_is_rate_limited_lower_case() {
        assert!(is_rate_limited("rate limit exceeded"));
    }

    #[test]
    fn test_is_rate_limited_mixed_case() {
        assert!(is_rate_limited("Too Many Requests"));
    }

    #[test]
    fn test_is_not_rate_limited() {
        assert!(!is_rate_limited("some random error"));
    }

    #[tokio::test]
    async fn test_retry_command_rate_limit_detection() {
        // Create a script that fails with rate-limit stderr twice, then succeeds.
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("rate_limit_mock.sh");
        let script_content = r#"#!/bin/bash
if [ -f /tmp/omk_retry_test_counter ]; then
    count=$(cat /tmp/omk_retry_test_counter)
else
    count=0
fi
count=$((count + 1))
echo "$count" > /tmp/omk_retry_test_counter
if [ "$count" -lt 3 ]; then
    echo "Error: 429 Too Many Requests" >&2
    exit 1
fi
echo "success"
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        // Clean up counter file before test
        let _ = tokio::fs::remove_file("/tmp/omk_retry_test_counter").await;

        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 10,
            rate_limit_delay_ms: 50,
            ..Default::default()
        };

        let mut cmd = tokio::process::Command::new(&script);
        let result = retry_command(config, &mut cmd).await;

        assert!(
            result.is_ok(),
            "Expected success after retries, got {:?}",
            result
        );
        assert_eq!(result.unwrap().trim(), "success");

        // Clean up
        let _ = tokio::fs::remove_file("/tmp/omk_retry_test_counter").await;
    }
}
