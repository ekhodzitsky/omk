use anyhow::{Context, Result};

use std::ffi::OsString;
use std::path::Path;
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use super::{
    GoalDeliveryPolicy, GoalGithubPrClient, GoalGithubPrCommandClient,
    GoalGithubPrRequest,
};
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::GoalDeliverySlice;

const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Options for delivering a slice PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicePrDeliveryOptions {
    pub policy: GoalDeliveryPolicy,
    pub dry_run: bool,
    pub base_branch: Option<String>,
}

/// Outcome of delivering a slice PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicePrDeliveryOutcome {
    pub commit_sha: Option<String>,
    pub pr_url: Option<String>,
    pub mutated: bool,
    pub reason: String,
}

/// Full pipeline: detect changes → commit → push → open/update PR for one slice.
pub async fn deliver_slice_pr(
    worktree_path: &Path,
    slice: &GoalDeliverySlice,
    goal_state: &GoalState,
    options: SlicePrDeliveryOptions,
) -> Result<SlicePrDeliveryOutcome> {
    if options.dry_run {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "dry-run: skipped slice PR delivery".to_string(),
        });
    }
    if !options.policy.permits_github_mutation() {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "local delivery policy does not permit GitHub mutation".to_string(),
        });
    }

    // Check if there are any changes to commit
    let has_changes = git_worktree_has_changes(worktree_path).await?;
    if !has_changes {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "no changes to commit in slice worktree".to_string(),
        });
    }

    let commit_sha = commit_slice_changes(worktree_path, slice, &goal_state.goal_id).await?;
    push_slice_branch(worktree_path, &slice.branch_name).await?;

    let outcome = open_slice_pr(slice, goal_state, &commit_sha, &options).await?;

    Ok(SlicePrDeliveryOutcome {
        commit_sha: Some(commit_sha),
        pr_url: outcome.pr_url.clone(),
        mutated: outcome.mutated,
        reason: outcome.reason,
    })
}

/// Auto-commit all changes in the slice worktree with a structured message.
pub async fn commit_slice_changes(
    worktree_path: &Path,
    slice: &GoalDeliverySlice,
    goal_id: &str,
) -> Result<String> {
    // Stage all changes
    let add_output = git_output(
        worktree_path,
        vec![OsString::from("add"), OsString::from("-A")],
        "stage slice changes",
    )
    .await?;
    if !add_output.status.success() {
        anyhow::bail!("git add failed: {}", output_stderr(&add_output));
    }

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
    let commit_output = git_output(
        worktree_path,
        vec![
            OsString::from("commit"),
            OsString::from("-m"),
            OsString::from(message),
        ],
        "commit slice changes",
    )
    .await?;
    if !commit_output.status.success() {
        anyhow::bail!("git commit failed: {}", output_stderr(&commit_output));
    }

    // Get the commit SHA
    let sha_output = git_output(
        worktree_path,
        vec![
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from("HEAD"),
        ],
        "get slice commit sha",
    )
    .await?;
    if !sha_output.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            output_stderr(&sha_output)
        );
    }

    Ok(output_stdout(&sha_output))
}

/// Push the slice branch to origin.
pub async fn push_slice_branch(worktree_path: &Path, branch: &str) -> Result<()> {
    let output = git_output(
        worktree_path,
        vec![
            OsString::from("push"),
            OsString::from("-u"),
            OsString::from("origin"),
            OsString::from(branch),
        ],
        "push slice branch",
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git push failed: {}", output_stderr(&output))
    }
}

/// Open or update a PR for a single slice.
pub async fn open_slice_pr(
    slice: &GoalDeliverySlice,
    goal_state: &GoalState,
    commit_sha: &str,
    options: &SlicePrDeliveryOptions,
) -> Result<SlicePrDeliveryOutcome> {
    let head_branch = slice.branch_name.clone();
    let title = format!(
        "[slice] {} — {}",
        slice.slice_id, goal_state.normalized_goal
    );
    let body = format!(
        "Slice `{}` for goal `{}`.\n\n- Owner: `{}`\n- Write scope: `{}`\n- Commit: `{}`\n- Slice dependencies: `{}`\n",
        slice.slice_id,
        goal_state.goal_id,
        slice.owner_role,
        slice.write_scope.join(", "),
        commit_sha,
        slice.dependencies.join(", "),
    );

    let request = GoalGithubPrRequest {
        title,
        body,
        head_branch,
        base_branch: options.base_branch.clone(),
        draft: options.policy == GoalDeliveryPolicy::DraftPr,
        existing_pr_url: slice.pr_url.clone(),
    };

    let mut client = GoalGithubPrCommandClient::default();
    let mutation = client.create_pr(request).await?;

    Ok(SlicePrDeliveryOutcome {
        commit_sha: Some(commit_sha.to_string()),
        pr_url: mutation.url.clone(),
        mutated: true,
        reason: format!("GitHub PR {} completed", mutation.operation.as_str()),
    })
}

async fn git_worktree_has_changes(worktree_path: &Path) -> Result<bool> {
    let output = git_output(
        worktree_path,
        vec![
            OsString::from("status"),
            OsString::from("--porcelain"),
        ],
        "check slice worktree for changes",
    )
    .await?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            output_stderr(&output)
        );
    }
    Ok(!output_stdout(&output).is_empty())
}

async fn git_output(worktree_path: &Path, args: Vec<OsString>, description: &str) -> Result<Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(worktree_path).args(args);
    timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| format!("Timed out while running git to {description}"))?
        .with_context(|| format!("Failed to run git to {description}"))
}

fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn output_stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    fn init_git_repo(path: &Path) {
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

    #[test]
    fn slice_pr_delivery_options_equality() {
        let a = SlicePrDeliveryOptions {
            policy: GoalDeliveryPolicy::DraftPr,
            dry_run: false,
            base_branch: Some("main".to_string()),
        };
        let b = SlicePrDeliveryOptions {
            policy: GoalDeliveryPolicy::DraftPr,
            dry_run: false,
            base_branch: Some("main".to_string()),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn slice_pr_delivery_outcome_local_policy_skips() {
        let outcome = SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "local delivery policy does not permit GitHub mutation".to_string(),
        };
        assert!(!outcome.mutated);
        assert!(outcome.commit_sha.is_none());
    }

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
