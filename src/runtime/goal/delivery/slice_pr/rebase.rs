use anyhow::{Context, Result};
use std::path::Path;

use crate::runtime::goal::git_ops::auto_rebase::{attempt_auto_rebase, RebaseOutcome};

use super::git::validate_git_ref;
use super::merge_check::check_slice_branch_merge_clean;

/// Ensure the slice branch can merge cleanly into the base branch.
/// If the branch is stale, attempt an auto-rebase onto the base.
/// Returns Ok(()) if clean (either originally or after rebase).
/// Returns Err if conflicts exist and auto-rebase failed.
pub(super) async fn ensure_slice_branch_merge_clean(
    worktree_path: &Path,
    branch: &str,
    base_branch: &str,
) -> Result<()> {
    validate_git_ref(branch)?;
    validate_git_ref(base_branch)?;

    // First attempt: check if merge is already clean
    if check_slice_branch_merge_clean(worktree_path, branch, base_branch)
        .await
        .is_ok()
    {
        return Ok(());
    }

    // Branch is stale or conflicting — try auto-rebase
    match attempt_auto_rebase(worktree_path, branch, base_branch).await {
        Ok(RebaseOutcome::Clean) => {}
        Ok(RebaseOutcome::ConflictUnresolvable) => {
            anyhow::bail!(
                "slice branch {branch} cannot merge cleanly into {base_branch} and auto-rebase failed: rebase conflicts could not be resolved"
            );
        }
        Err(e) => {
            anyhow::bail!(
                "slice branch {branch} cannot merge cleanly into {base_branch} and auto-rebase failed: {e}"
            );
        }
    }

    // Rebase succeeded — re-check merge-tree
    check_slice_branch_merge_clean(worktree_path, branch, base_branch)
        .await
        .with_context(|| {
            format!(
            "slice branch {branch} still has merge conflicts after auto-rebase onto {base_branch}"
        )
        })
}

#[cfg(test)]
mod tests {
    use super::super::git::tests::init_git_repo;
    use super::*;
    use std::process::Command as StdCommand;

    #[tokio::test]
    async fn ensure_slice_branch_merge_clean_passes_for_clean_branch() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir");
        init_git_repo(&repo);

        // Create a branch from main with no conflicts
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["checkout", "-b", "feature"])
            .output()
            .expect("checkout feature");
        std::fs::write(repo.join("feature.txt"), "feature").expect("write");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .output()
            .expect("git add");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "feature"])
            .output()
            .expect("git commit");

        ensure_slice_branch_merge_clean(&repo, "feature", "master")
            .await
            .expect("clean branch should pass merge check");
    }

    #[tokio::test]
    async fn ensure_slice_branch_merge_clean_rebases_stale_branch() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir");
        init_git_repo(&repo);

        // Create a feature branch
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["checkout", "-b", "feature"])
            .output()
            .expect("checkout feature");
        std::fs::write(repo.join("feature.txt"), "feature").expect("write");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .output()
            .expect("git add");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "feature"])
            .output()
            .expect("git commit");

        // Go back to master and add a new commit (making feature stale)
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["checkout", "master"])
            .output()
            .expect("checkout master");
        std::fs::write(repo.join("master.txt"), "master").expect("write");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .output()
            .expect("git add");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "master update"])
            .output()
            .expect("git commit");

        // The feature branch is now stale but has no conflicts
        ensure_slice_branch_merge_clean(&repo, "feature", "master")
            .await
            .expect("stale branch should be auto-rebased and pass");
    }

    #[tokio::test]
    async fn ensure_slice_branch_merge_clean_fails_for_conflicting_branch() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir");
        init_git_repo(&repo);

        // Create a feature branch that modifies the same file as master will
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["checkout", "-b", "feature"])
            .output()
            .expect("checkout feature");
        std::fs::write(repo.join("shared.txt"), "feature content").expect("write");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .output()
            .expect("git add");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "feature"])
            .output()
            .expect("git commit");

        // Go back to master and modify the same file differently
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["checkout", "master"])
            .output()
            .expect("checkout master");
        std::fs::write(repo.join("shared.txt"), "master content").expect("write");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .output()
            .expect("git add");
        StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "master update"])
            .output()
            .expect("git commit");

        // The feature branch has real conflicts — auto-rebase should fail
        let result = ensure_slice_branch_merge_clean(&repo, "feature", "master").await;
        assert!(
            result.is_err(),
            "conflicting branch should fail merge check even after auto-rebase attempt"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("auto-rebase failed") || err.contains("still has merge conflicts"),
            "error should mention rebase or conflict failure: {err}"
        );
    }
}
