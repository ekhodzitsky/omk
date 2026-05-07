#![allow(dead_code)]

use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, info, warn};

pub fn ensure_tmux() -> Result<()> {
    match Command::new("tmux").arg("-V").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            info!(tmux_version = %version, "tmux detected");
            Ok(())
        }
        _ => anyhow::bail!(
            "tmux is required but not installed.\n\
             Install: brew install tmux (macOS), sudo apt install tmux (Ubuntu)"
        ),
    }
}

pub fn session_exists(name: &str) -> Result<bool> {
    let output = Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()?;
    Ok(output.status.success())
}

pub fn create_session(name: &str, window_name: &str, cwd: &std::path::Path) -> Result<()> {
    let mut cmd = Command::new("tmux");
    cmd.args([
        "new-session",
        "-d",
        "-s",
        name,
        "-n",
        window_name,
    ]);
    
    if cwd != std::path::Path::new(".") && cwd.exists() {
        cmd.arg("-c").arg(cwd);
    }

    let output = cmd.output().context("Failed to create tmux session")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux new-session failed: {}", stderr);
    }

    info!(session = name, window = window_name, "Created tmux session");
    Ok(())
}

pub fn split_window(session: &str, window: &str, cwd: &std::path::Path) -> Result<()> {
    let target = format!("{}:{}", session, window);
    let mut cmd = Command::new("tmux");
    cmd.args(["split-window", "-t", &target]);

    if cwd != std::path::Path::new(".") && cwd.exists() {
        cmd.arg("-c").arg(cwd);
    }

    let output = cmd.output().context("Failed to split tmux window")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(target = %target, error = %stderr, "tmux split-window failed");
    }
    Ok(())
}

pub fn rename_pane(session: &str, window: &str, pane_index: usize, name: &str) -> Result<()> {
    let target = format!("{}:{}.{}", session, window, pane_index);
    let output = Command::new("tmux")
        .args(["select-pane", "-t", &target, "-T", name])
        .output()?;
    
    if !output.status.success() {
        debug!(target = %target, name = name, "Pane rename may not be supported by this tmux version");
    }
    Ok(())
}

pub fn send_keys(session: &str, window: &str, keys: &str) -> Result<()> {
    let target = format!("{}:{}", session, window);
    let output = Command::new("tmux")
        .args(["send-keys", "-t", &target, keys, "Enter"])
        .output()
        .context("Failed to send keys to tmux")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux send-keys failed: {}", stderr);
    }

    debug!(target = %target, keys = %keys, "Sent keys to tmux");
    Ok(())
}

pub fn select_layout(session: &str, window: &str, layout: &str) -> Result<()> {
    let target = format!("{}:{}", session, window);
    let output = Command::new("tmux")
        .args(["select-layout", "-t", &target, layout])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(target = %target, layout = layout, error = %stderr, "tmux select-layout failed");
    }
    Ok(())
}

pub fn kill_session(name: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux kill-session failed: {}", stderr);
    }

    info!(session = name, "Killed tmux session");
    Ok(())
}

pub fn list_panes(session: &str, window: &str) -> Result<Vec<String>> {
    let target = format!("{}:{}", session, window);
    let output = Command::new("tmux")
        .args(["list-panes", "-t", &target, "-F", "#{pane_index}:#{pane_title}"])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let lines = String::from_utf8_lossy(&output.stdout);
    Ok(lines.lines().map(|s| s.to_string()).collect())
}
