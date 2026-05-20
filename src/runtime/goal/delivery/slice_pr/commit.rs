use anyhow::Result;
use std::path::Path;

use crate::git::GitRepo;
use crate::runtime::goal::task_graph::GoalDeliverySlice;

/// Auto-commit all changes in the slice worktree with a structured message.
pub(super) async fn commit_slice_changes(
    worktree_path: &Path,
    slice: &GoalDeliverySlice,
    goal_id: &str,
) -> Result<String> {
    let repo = GitRepo::open(worktree_path)
        .map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;

    // Stage all changes
    repo.add_all()
        .await
        .map_err(|e| anyhow::anyhow!("git add failed: {e}"))?;

    // Build commit message
    let write_scope_text = if slice.write_scope.is_empty() {
        "project files".to_string()
    } else {
        slice.write_scope.join(", ")
    };
    let message = format!(
        "[omk-slice] {goal_id} / {}\n\nWrite scope: {write_scope_text}",
        slice.slice_id
    );

    // Commit
    repo.commit(&message, &[] as &[&std::path::Path])
        .await
        .map_err(|e| anyhow::anyhow!("git commit failed: {e}"))?;

    // Return the commit SHA
    repo.head_commit()
        .await
        .map_err(|e| anyhow::anyhow!("git rev-parse failed: {e}"))
}

/// Push the slice branch to origin.
pub(super) async fn push_slice_branch(worktree_path: &Path, branch: &str) -> Result<()> {
    let repo = GitRepo::open(worktree_path)
        .map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    repo.push(super::DEFAULT_REMOTE, branch, false)
        .await
        .map_err(|e| anyhow::anyhow!("git push failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::git::tests::init_git_repo;
    use super::*;
    use crate::runtime::goal::task_graph::GoalDeliverySlice;
    use std::process::Command as StdCommand;

    #[tokio::test]
    async fn commit_slice_changes_creates_commit_with_structured_message() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir");
        init_git_repo(&repo);

        // Create a file to commit
        std::fs::write(repo.join("hello.txt"), "world").expect("write");

        let slice = GoalDeliverySlice {
            slice_id: "slice-1".to_string(),
            task_id: "t1".to_string(),
            owner_role: "executor".to_string(),
            read_scope: vec![],
            write_scope: vec!["src".to_string()],
            dependencies: vec![],
            branch_name: "test-branch".to_string(),
            worktree_name: "wt".to_string(),
            worktree_path: repo.clone(),
            gates: vec![],
            review_needs: vec![],
            pr_url: None,
        };

        let sha = commit_slice_changes(&repo, &slice, "goal-123")
            .await
            .expect("commit_slice_changes");
        assert!(!sha.is_empty(), "commit sha should not be empty");

        // Verify the commit message
        let output = StdCommand::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["log", "-1", "--pretty=%B"])
            .output()
            .expect("git log");
        let message = String::from_utf8_lossy(&output.stdout);
        assert!(message.contains("[omk-slice] goal-123 / slice-1"));
        assert!(message.contains("Write scope: src"));
    }
}
