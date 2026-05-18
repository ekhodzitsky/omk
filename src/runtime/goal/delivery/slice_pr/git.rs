use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::Path;
use std::process::Output;

use tokio::process::Command;
use tokio::time::timeout;

use super::GIT_COMMAND_TIMEOUT;

/// Run a git command in the given worktree and return the output.
pub(super) async fn git_output(
    worktree_path: &Path,
    args: Vec<OsString>,
    description: &str,
) -> Result<Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(worktree_path).args(args);
    timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| format!("Timed out while running git to {description}"))?
        .with_context(|| format!("Failed to run git to {description}"))
}

/// Check whether the worktree has any uncommitted changes.
pub(super) async fn git_worktree_has_changes(worktree_path: &Path) -> Result<bool> {
    let output = git_output(
        worktree_path,
        vec![OsString::from("status"), OsString::from("--porcelain")],
        "check slice worktree for changes",
    )
    .await?;
    if !output.status.success() {
        anyhow::bail!("git status failed: {}", output_stderr(&output));
    }
    Ok(!output_stdout(&output).is_empty())
}

/// Extract stdout as a trimmed string.
pub(super) fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Extract stderr as a trimmed string.
pub(super) fn output_stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

/// Reject git ref names that start with `-` to prevent argument injection.
pub(super) fn validate_git_ref(name: &str) -> Result<()> {
    if name.starts_with('-') {
        anyhow::bail!("invalid git ref name: cannot start with '-': {name}");
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    pub fn init_git_repo(path: &Path) {
        StdCommand::new("git")
            .arg("-C")
            .arg(path)
            .arg("init")
            .output()
            .expect("git init");
        StdCommand::new("git")
            .arg("-C")
            .arg(path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .expect("git config email");
        StdCommand::new("git")
            .arg("-C")
            .arg(path)
            .args(["config", "user.name", "Test"])
            .output()
            .expect("git config name");
        std::fs::write(path.join("baseline.txt"), "baseline\n").expect("write baseline");
        StdCommand::new("git")
            .arg("-C")
            .arg(path)
            .args(["add", "."])
            .output()
            .expect("git add");
        StdCommand::new("git")
            .arg("-C")
            .arg(path)
            .args(["commit", "-m", "baseline"])
            .output()
            .expect("git commit");
    }

    #[tokio::test]
    async fn git_worktree_has_changes_detects_untracked_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir");
        init_git_repo(&repo);

        // No changes initially
        assert!(!git_worktree_has_changes(&repo).await.expect("check"));

        // Create a file
        std::fs::write(repo.join("new.txt"), "content").expect("write");
        assert!(git_worktree_has_changes(&repo).await.expect("check"));
    }
}
