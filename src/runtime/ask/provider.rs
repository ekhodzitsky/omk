use anyhow::Result;
use std::time::Duration;

pub const ALL_PROVIDERS: &[&str] = &["claude", "codex", "gemini", "kimi"];

/// Return the list of providers whose CLI binary is found on PATH.
pub async fn available_providers() -> Vec<&'static str> {
    let mut providers = Vec::new();
    for &p in ALL_PROVIDERS {
        if is_provider_installed(p).await {
            providers.push(p);
        }
    }
    providers
}

/// Check whether a provider CLI is installed.
pub async fn is_provider_installed(provider: &str) -> bool {
    match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("which").arg(provider).output(),
    )
    .await
    {
        Ok(Ok(out)) => out.status.success(),
        _ => false,
    }
}

/// Return the shell command string for a given provider and prompt.
pub fn provider_command(provider: &str, prompt: &str) -> Result<String> {
    let escaped = shell_escape(prompt)?;
    match provider {
        "claude" => Ok(format!("claude -p {escaped}")),
        "codex" => Ok(format!("codex -p {escaped}")),
        "gemini" => Ok(format!("gemini -p {escaped}")),
        "kimi" => Ok(format!("kimi -p {escaped}")),
        _ => anyhow::bail!("Unknown provider: {}", provider),
    }
}

/// Check whether a string is one of the known advisor providers.
pub fn is_known_provider(name: &str) -> bool {
    ALL_PROVIDERS.contains(&name)
}

/// Escape a string for safe inclusion in a single-quoted shell context.
fn shell_escape(s: &str) -> Result<String> {
    crate::runtime::shell::shell_escape(s)
}
