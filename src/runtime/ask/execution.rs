use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tracing::debug;

use super::provider::{is_known_provider, is_provider_installed};

/// Run a provider directly and capture its stdout+stderr.
/// Used for the MVP direct-execution path and for synthesis.
///
/// Defense in depth: this path spawns the provider binary directly via
/// `Command::new(provider).arg("-p").arg(prompt)` instead of routing through
/// `bash -c`. The prompt is delivered as a single argv element, so a
/// hostile prompt cannot break out into shell metacharacters even if the
/// upstream shell-escape helper were to regress. The provider name is
/// constrained to `ALL_PROVIDERS` up front so this argv-mode invocation
/// can never call an arbitrary binary either.
pub async fn run_advisor_direct(provider: &str, prompt: &str, timeout_secs: u64) -> Result<String> {
    if !is_known_provider(provider) {
        anyhow::bail!("Unknown provider: {}", provider);
    }
    if !is_provider_installed(provider).await {
        anyhow::bail!("Provider '{}' is not installed", provider);
    }

    debug!(
        provider = provider,
        "Running advisor directly via argv (no shell)"
    );

    let mut child = tokio::process::Command::new(provider)
        .arg("-p")
        .arg(prompt)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("Failed to spawn {}", provider))?;

    let output = match tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait()).await {
        Ok(Ok(status)) => {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = tokio::io::AsyncReadExt::read_to_end(&mut out, &mut stdout).await;
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = tokio::io::AsyncReadExt::read_to_end(&mut err, &mut stderr).await;
            }
            std::process::Output {
                status,
                stdout,
                stderr,
            }
        }
        Ok(Err(e)) => {
            return Err(anyhow::anyhow!("Failed to run {}: {}", provider, e));
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            anyhow::bail!("{} timed out after {}s", provider, timeout_secs);
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        anyhow::bail!(
            "{} exited with status {:?}\nstdout: {}\nstderr: {}",
            provider,
            output.status.code(),
            stdout,
            stderr
        );
    }

    Ok(format!("{}{}", stdout, stderr).trim().to_string())
}

/// Poll an outbox file until the done marker appears or a timeout is reached.
pub async fn poll_outbox(outbox: &Path, timeout_secs: u64) -> Result<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("Timeout waiting for advisor output");
        }

        if outbox.exists() {
            let content = tokio::fs::read_to_string(outbox).await?;
            if content.contains("___OMK_ASK_DONE___") {
                let cleaned = content.replace("___OMK_ASK_DONE___", "").trim().to_string();
                return Ok(cleaned);
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
