use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

pub const ALL_PROVIDERS: &[&str] = &["claude", "codex", "gemini", "kimi"];

/// Return the list of providers whose CLI binary is found on PATH.
pub fn available_providers() -> Vec<&'static str> {
    ALL_PROVIDERS
        .iter()
        .copied()
        .filter(|p| is_provider_installed(p))
        .collect()
}

/// Check whether a provider CLI is installed.
pub fn is_provider_installed(provider: &str) -> bool {
    Command::new("which")
        .arg(provider)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Return the shell command string for a given provider and prompt.
pub fn provider_command(provider: &str, prompt: &str) -> Result<String> {
    let escaped = shell_escape(prompt);
    match provider {
        "claude" => Ok(format!("claude -p {escaped}")),
        "codex" => Ok(format!("codex -p {escaped}")),
        "gemini" => Ok(format!("gemini -p {escaped}")),
        "kimi" => Ok(format!("kimi -p {escaped}")),
        _ => anyhow::bail!("Unknown provider: {}", provider),
    }
}

/// Directory where ask artifacts are persisted.
pub fn artifact_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("No home directory")?;
    Ok(home.join(".omk").join("artifacts").join("ask"))
}

/// Full path for a named artifact at a given timestamp.
pub fn artifact_path(name: &str, timestamp: &str) -> Result<PathBuf> {
    let dir = artifact_dir()?;
    Ok(dir.join(format!("{}-{name}.md", timestamp)))
}

/// Save an artifact to a specific base directory (useful for testing).
pub async fn save_artifact_to(
    base_dir: &Path,
    name: &str,
    content: &str,
    timestamp: &str,
) -> Result<PathBuf> {
    let path = base_dir.join(format!("{}-{name}.md", timestamp));
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, content).await?;
    info!(path = %path.display(), name = name, "Saved artifact");
    Ok(path)
}

/// Save an artifact to the default `.omk/artifacts/ask` directory.
pub async fn save_artifact(name: &str, content: &str, timestamp: &str) -> Result<PathBuf> {
    let dir = artifact_dir()?;
    save_artifact_to(&dir, name, content, timestamp).await
}

/// Run a provider directly (not inside tmux) and capture its stdout+stderr.
/// Used for the MVP direct-execution path and for synthesis.
pub async fn run_advisor_direct(provider: &str, prompt: &str) -> Result<String> {
    if !is_provider_installed(provider) {
        anyhow::bail!("Provider '{}' is not installed", provider);
    }

    let cmd = provider_command(provider, prompt)?;
    debug!(provider = provider, cmd = %cmd, "Running advisor directly");

    let output = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        .output()
        .await
        .with_context(|| format!("Failed to run {}", provider))?;

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

/// Spawn an advisor inside a tmux session/window.
/// The provider's stdout+stderr are redirected to `outbox`.
pub async fn spawn_advisor_tmux(
    session: &str,
    window: &str,
    provider: &str,
    prompt: &str,
    outbox: &Path,
) -> Result<()> {
    if !is_provider_installed(provider) {
        anyhow::bail!("Provider '{}' is not installed", provider);
    }

    let cmd = provider_command(provider, prompt)?;
    let outbox_str = outbox.to_string_lossy();
    let parent_str = outbox.parent().unwrap_or_else(|| Path::new(".")).to_string_lossy();

    let wrapper = format!(
        "mkdir -p {} && {} > {} 2>&1 && echo '___OMK_ASK_DONE___' >> {}",
        shell_escape(&parent_str),
        cmd,
        shell_escape(&outbox_str),
        shell_escape(&outbox_str),
    );

    crate::runtime::tmux::send_keys(session, window, &wrapper)?;
    info!(
        provider = provider,
        session = session,
        outbox = %outbox.display(),
        "Spawned advisor in tmux"
    );
    Ok(())
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

/// Query a single provider and optionally persist the artifact.
pub async fn ask_single(provider: &str, prompt: &str, save: bool) -> Result<String> {
    let output = run_advisor_direct(provider, prompt).await?;

    if save {
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let content = format!(
            "# {provider} answer\n\nPrompt: {prompt}\n\n---\n\n{output}\n"
        );
        save_artifact(provider, &content, &ts).await?;
    }

    Ok(output)
}

/// Query every available provider in parallel.
/// Returns a vec of `(provider, output)` pairs.
pub async fn ask_all(prompt: &str, save: bool) -> Result<Vec<(String, String)>> {
    let providers = available_providers();
    if providers.is_empty() {
        anyhow::bail!("No provider CLIs are installed");
    }

    info!(providers = ?providers, "Querying all advisors");
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();

    let mut tasks = tokio::task::JoinSet::new();
    for provider in providers {
        let p = prompt.to_string();
        tasks.spawn(async move {
            match run_advisor_direct(provider, &p).await {
                Ok(output) => Ok((provider.to_string(), output)),
                Err(e) => Err((provider.to_string(), e)),
            }
        });
    }

    let mut results = Vec::new();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok((provider, output))) => {
                if save {
                    let content = format!(
                        "# {provider} answer\n\nPrompt: {prompt}\n\n---\n\n{output}\n"
                    );
                    let _ = save_artifact(&provider, &content, &ts).await;
                }
                results.push((provider, output));
            }
            Ok(Err((provider, e))) => {
                warn!(provider = provider, error = %e, "Advisor failed");
            }
            Err(e) => {
                warn!(error = %e, "Advisor task panicked");
            }
        }
    }

    if results.is_empty() {
        anyhow::bail!("All advisors failed to produce output");
    }

    Ok(results)
}

/// Build the synthesis prompt fed to Kimi.
pub fn build_synthesis_prompt(prompt: &str, outputs: &[(String, String)]) -> String {
    let mut synthesis = format!(
        "The user asked: {}\n\nHere are answers from multiple AI advisors:\n\n",
        prompt
    );
    for (provider, output) in outputs {
        synthesis.push_str(&format!("## {}\n\n{}\n\n", provider, output));
    }
    synthesis.push_str(
        "Please synthesize a unified answer that combines the best insights from all advisors. \
         Be concise but thorough.\n",
    );
    synthesis
}

/// Synthesize multiple advisor outputs into a unified answer using Kimi.
pub async fn synthesize(
    prompt: &str,
    outputs: &[(String, String)],
    save: bool,
) -> Result<String> {
    if !is_provider_installed("kimi") {
        anyhow::bail!("Kimi CLI is required for synthesis but is not installed");
    }

    let synthesis_prompt = build_synthesis_prompt(prompt, outputs);
    let result = run_advisor_direct("kimi", &synthesis_prompt).await?;

    if save {
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let content = format!(
            "# Synthesis\n\nOriginal prompt: {prompt}\n\n---\n\n{result}\n"
        );
        save_artifact("synthesis", &content, &ts).await?;
    }

    Ok(result)
}

/// Check whether a string is one of the known advisor providers.
pub fn is_known_provider(name: &str) -> bool {
    ALL_PROVIDERS.contains(&name)
}

/// Escape a string for safe inclusion in a single-quoted shell context.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_detection() {
        assert!(is_provider_installed("bash"));
        assert!(!is_provider_installed("definitely_not_a_real_binary_12345"));
    }

    #[tokio::test]
    async fn test_artifact_path_generation() {
        let path = artifact_path("claude", "20260507-121530").unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("20260507-121530"));
        assert!(name.contains("claude"));
        assert!(name.ends_with(".md"));
    }

    #[tokio::test]
    async fn test_synthesis_prompt_building() {
        let outputs = vec![
            ("claude".to_string(), "Claude answer".to_string()),
            ("kimi".to_string(), "Kimi answer".to_string()),
        ];
        let prompt = build_synthesis_prompt("What is Rust?", &outputs);
        assert!(prompt.contains("What is Rust?"));
        assert!(prompt.contains("Claude answer"));
        assert!(prompt.contains("Kimi answer"));
        assert!(prompt.contains("synthesize"));
    }

    #[tokio::test]
    async fn test_save_artifact_to() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("artifacts").join("ask");
        let path = save_artifact_to(&base, "claude", "test content", "20260507-121530")
            .await
            .unwrap();
        assert!(path.exists());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn test_run_advisor_direct_with_mock() {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join("mock-advisor");
        std::fs::write(&script_path, "#!/bin/bash\necho 'mock output'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();
        }

        let original_path = std::env::var_os("PATH");
        let mut new_path = std::ffi::OsString::from(dir.path());
        new_path.push(":");
        new_path.push(original_path.clone().unwrap_or_default());
        std::env::set_var("PATH", &new_path);

        let result = run_advisor_direct("mock-advisor", "test prompt").await;

        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "mock output");
    }

    #[tokio::test]
    async fn test_provider_command_generation() {
        assert_eq!(
            provider_command("kimi", "hello").unwrap(),
            "kimi -p 'hello'"
        );
        assert_eq!(
            provider_command("claude", "it's working").unwrap(),
            "claude -p 'it'\"'\"'s working'"
        );
    }

    #[tokio::test]
    async fn test_is_known_provider() {
        assert!(is_known_provider("kimi"));
        assert!(is_known_provider("claude"));
        assert!(!is_known_provider("gpt4"));
        assert!(!is_known_provider(""));
    }
}
