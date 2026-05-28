use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub(crate) enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Unknown,
}

pub(crate) fn detect_project_type(dir: &Path) -> ProjectType {
    if dir.join("Cargo.toml").exists() {
        ProjectType::Rust
    } else if dir.join("package.json").exists() {
        ProjectType::Node
    } else if dir.join("go.mod").exists() {
        ProjectType::Go
    } else if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("requirements.txt").exists()
    {
        ProjectType::Python
    } else {
        ProjectType::Unknown
    }
}

pub(crate) async fn run_kimi_prompt(prompt: &str) -> Result<String> {
    let output = crate::runtime::shell::run_kimi(prompt, None, true, std::time::Duration::from_secs(120)).await
        .context("kimi prompt timed out")?;
    if !output.status.success() {
        anyhow::bail!("kimi exited with non-zero status");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) async fn run_command(dir: &Path, cmd: &str, args: &[&str]) -> Result<()> {
    let mut command = Command::new(cmd);
    command.args(args).current_dir(dir);
    crate::runtime::shell::configure_command(&mut command);
    let output = tokio::time::timeout(std::time::Duration::from_secs(60), command.output())
        .await
        .with_context(|| format!("{} {} timed out", cmd, args.join(" ")))?
        .with_context(|| format!("Failed to run {} {}", cmd, args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command failed: {} {}\n{}", cmd, args.join(" "), stderr);
    }

    Ok(())
}

// shell_escape: use crate::runtime::shell::shell_escape directly
