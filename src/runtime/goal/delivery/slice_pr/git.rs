use anyhow::Result;
use std::path::Path;

use crate::git::GitRepo;

/// Check whether the worktree has any uncommitted changes.
pub(super) async fn git_worktree_has_changes(worktree_path: &Path) -> Result<bool> {
    let repo = GitRepo::open(worktree_path)
        .map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    let files = repo
        .changed_files()
        .await
        .map_err(|e| anyhow::anyhow!("git status failed: {e}"))?;
    Ok(!files.is_empty())
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
    use std::path::Path;
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
