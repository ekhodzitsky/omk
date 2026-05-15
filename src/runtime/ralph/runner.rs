// Ralph external process runners.
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use tokio::process::Command;
use tracing::warn;

/// Spawn `kimi -p` and capture its combined output.
pub async fn run_kimi(prompt: &str, dir: &Path) -> Result<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        Command::new("kimi")
            .args(["-p", prompt])
            .current_dir(dir)
            .output(),
    )
    .await??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        warn!(status = ?output.status, stderr = %stderr, "kimi command failed");
    }

    Ok(format!("{}{}", stdout, stderr))
}

/// Run `cargo test` in the given directory and return whether it succeeded.
pub async fn run_tests(dir: &Path) -> Result<bool> {
    let output = tokio::time::timeout(
        Duration::from_secs(300),
        Command::new("cargo")
            .args(["test", "--quiet"])
            .current_dir(dir)
            .output(),
    )
    .await??;

    Ok(output.status.success())
}
