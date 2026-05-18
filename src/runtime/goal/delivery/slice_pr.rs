use anyhow::{Context, Result};

use std::ffi::OsString;
use std::path::Path;
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use super::{
    GoalDeliveryPolicy, GoalGithubPrClient, GoalGithubPrCommandClient, GoalGithubPrOperation,
    GoalGithubPrRequest,
};
use crate::runtime::goal::review::{
    anti_slop_confidence, review_slice, ANTI_SLOP_ACTIONABLE_THRESHOLD,
};
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTaskGraph};

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
    pub review_artifacts: Option<Vec<crate::runtime::goal::review::SliceReviewArtifact>>,
}

/// Full pipeline: detect changes → commit → push → open/update PR for one slice.
pub async fn deliver_slice_pr(
    worktree_path: &Path,
    slice: &GoalDeliverySlice,
    goal_state: &GoalState,
    task_graph: &GoalTaskGraph,
    options: SlicePrDeliveryOptions,
) -> Result<SlicePrDeliveryOutcome> {
    if options.dry_run {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "dry-run: skipped slice PR delivery".to_string(),
            review_artifacts: None,
        });
    }
    if !options.policy.permits_github_mutation() {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "local delivery policy does not permit GitHub mutation".to_string(),
            review_artifacts: None,
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
            review_artifacts: None,
        });
    }

    let commit_sha = commit_slice_changes(worktree_path, slice, &goal_state.goal_id).await?;

    // Run the 6-review wall before opening the PR.
    let review = review_slice(slice, goal_state, task_graph, worktree_path).await?;
    if !review.passed {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: Some(commit_sha),
            pr_url: None,
            mutated: false,
            reason: format!(
                "slice review wall blocked: {}",
                review.feedback.unwrap_or_default()
            ),
            review_artifacts: Some(review.artifacts),
        });
    }
    let anti_slop_conf = anti_slop_confidence(&review.artifacts);
    if anti_slop_conf > ANTI_SLOP_ACTIONABLE_THRESHOLD {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: Some(commit_sha),
            pr_url: None,
            mutated: false,
            reason: format!(
                "slice blocked by anti-slop confidence {:.2} exceeding threshold",
                anti_slop_conf
            ),
            review_artifacts: Some(review.artifacts),
        });
    }

    let base_branch = options.base_branch.as_deref().unwrap_or("main");
    if let Err(e) =
        ensure_slice_branch_merge_clean(worktree_path, &slice.branch_name, base_branch).await
    {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: Some(commit_sha),
            pr_url: None,
            mutated: false,
            reason: format!("slice branch merge check failed: {e}"),
            review_artifacts: Some(review.artifacts),
        });
    }

    push_slice_branch(worktree_path, &slice.branch_name).await?;

    let outcome = open_slice_pr(slice, goal_state, &commit_sha, &options).await?;

    Ok(SlicePrDeliveryOutcome {
        commit_sha: Some(commit_sha),
        pr_url: outcome.pr_url.clone(),
        mutated: outcome.mutated,
        reason: outcome.reason,
        review_artifacts: Some(review.artifacts),
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
        anyhow::bail!("git rev-parse failed: {}", output_stderr(&sha_output));
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
    let operation = if request.existing_pr_url.is_some() {
        GoalGithubPrOperation::Update
    } else {
        GoalGithubPrOperation::Create
    };
    let mutation = match operation {
        GoalGithubPrOperation::Create => client.create_pr(request).await?,
        GoalGithubPrOperation::Update => client.update_pr(request).await?,
    };

    Ok(SlicePrDeliveryOutcome {
        commit_sha: Some(commit_sha.to_string()),
        pr_url: mutation.url.clone(),
        mutated: true,
        reason: format!("GitHub PR {} completed", mutation.operation.as_str()),
        review_artifacts: None,
    })
}

async fn git_worktree_has_changes(worktree_path: &Path) -> Result<bool> {
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

async fn git_output(
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

fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn validate_git_ref(name: &str) -> Result<()> {
    if name.starts_with('-') {
        anyhow::bail!("invalid git ref name: cannot start with '-': {name}");
    }
    Ok(())
}

/// Ensure the slice branch can merge cleanly into the base branch.
/// If the branch is stale, attempt an auto-rebase onto the base.
/// Returns Ok(()) if clean (either originally or after rebase).
/// Returns Err if conflicts exist and auto-rebase failed.
async fn ensure_slice_branch_merge_clean(
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
    if let Err(e) = rebase_slice_branch_onto_base(worktree_path, branch, base_branch).await {
        anyhow::bail!(
            "slice branch {branch} cannot merge cleanly into {base_branch} and auto-rebase failed: {e}"
        );
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

/// Check whether the slice branch merges cleanly into the base branch.
/// Uses a temporary `git merge --no-commit --no-ff` so the working tree
/// is not permanently altered. Returns Ok if clean, Err if conflicts are predicted.
async fn check_slice_branch_merge_clean(
    worktree_path: &Path,
    branch: &str,
    base_branch: &str,
) -> Result<()> {
    // Stash any uncommitted changes so we can safely switch branches.
    let has_local_changes = git_worktree_has_changes(worktree_path)
        .await
        .unwrap_or(true);
    let stash = if has_local_changes {
        git_output(
            worktree_path,
            vec![
                OsString::from("stash"),
                OsString::from("push"),
                OsString::from("-u"),
                OsString::from("-m"),
                OsString::from("omk-merge-check"),
            ],
            "stash changes for merge check",
        )
        .await
    } else {
        Err(anyhow::anyhow!("no local changes to stash"))
    };

    let original_branch = git_output(
        worktree_path,
        vec![OsString::from("branch"), OsString::from("--show-current")],
        "get current branch",
    )
    .await?;
    let original_branch = output_stdout(&original_branch);
    if original_branch.is_empty() {
        // Pop stash before bailing so we do not leave the worktree dirty.
        if stash.map(|o| o.status.success()).unwrap_or(false) {
            let _ = git_output(
                worktree_path,
                vec![OsString::from("stash"), OsString::from("pop")],
                "pop stash after detached-head bail",
            )
            .await;
        }
        anyhow::bail!("cannot determine current branch for merge check");
    }

    // Fetch latest base branch from origin (best-effort).
    let _ = git_output(
        worktree_path,
        vec![
            OsString::from("fetch"),
            OsString::from("origin"),
            OsString::from(base_branch),
        ],
        "fetch base branch for merge check",
    )
    .await;

    let base_ref = base_branch.to_string();

    // Checkout base branch.
    let checkout_base = git_output(
        worktree_path,
        vec![OsString::from("checkout"), OsString::from(&base_ref)],
        "checkout base branch for merge check",
    )
    .await?;
    if !checkout_base.status.success() {
        let _ = git_output(
            worktree_path,
            vec![OsString::from("checkout"), OsString::from(&original_branch)],
            "restore original branch after failed checkout",
        )
        .await;
        if stash.map(|o| o.status.success()).unwrap_or(false) {
            let _ = git_output(
                worktree_path,
                vec![OsString::from("stash"), OsString::from("pop")],
                "pop stash after failed checkout",
            )
            .await;
        }
        anyhow::bail!(
            "git checkout {base_ref} failed: {}",
            output_stderr(&checkout_base)
        );
    }

    // Attempt a test merge.
    let merge = git_output(
        worktree_path,
        vec![
            OsString::from("merge"),
            OsString::from("--no-commit"),
            OsString::from("--no-ff"),
            OsString::from("--"),
            OsString::from(branch),
        ],
        "test merge for conflicts",
    )
    .await?;

    // Check for unmerged (conflicted) files.
    let diff = git_output(
        worktree_path,
        vec![
            OsString::from("diff"),
            OsString::from("--cached"),
            OsString::from("--name-only"),
            OsString::from("--diff-filter=U"),
        ],
        "check for merge conflicts",
    )
    .await?;
    let conflicts = output_stdout(&diff);

    // Abort the test merge (best-effort; may fail if merge did not start).
    let _ = git_output(
        worktree_path,
        vec![OsString::from("merge"), OsString::from("--abort")],
        "abort test merge",
    )
    .await;

    // Restore original branch.
    let restore_branch = git_output(
        worktree_path,
        vec![OsString::from("checkout"), OsString::from(&original_branch)],
        "restore original branch",
    )
    .await?;
    if !restore_branch.status.success() {
        // Try to pop stash before reporting the restore failure.
        if stash.map(|o| o.status.success()).unwrap_or(false) {
            let _ = git_output(
                worktree_path,
                vec![OsString::from("stash"), OsString::from("pop")],
                "pop stash after failed branch restore",
            )
            .await;
        }
        anyhow::bail!(
            "failed to restore original branch {} after merge check: {}",
            original_branch,
            output_stderr(&restore_branch)
        );
    }

    // Pop stash if we created one.
    if stash.map(|o| o.status.success()).unwrap_or(false) {
        let pop = git_output(
            worktree_path,
            vec![OsString::from("stash"), OsString::from("pop")],
            "pop stash",
        )
        .await?;
        if !pop.status.success() {
            anyhow::bail!(
                "failed to pop stash after merge check: {}",
                output_stderr(&pop)
            );
        }
    }

    if !conflicts.is_empty() {
        anyhow::bail!("merge predicts conflicts in files: {conflicts}");
    }

    if !merge.status.success() {
        anyhow::bail!("git merge failed: {}", output_stderr(&merge));
    }

    Ok(())
}

/// Attempt to rebase the slice branch onto the latest base branch.
async fn rebase_slice_branch_onto_base(
    worktree_path: &Path,
    branch: &str,
    base_branch: &str,
) -> Result<()> {
    validate_git_ref(branch)?;
    validate_git_ref(base_branch)?;

    // Checkout the slice branch
    let checkout = git_output(
        worktree_path,
        vec![OsString::from("checkout"), OsString::from(branch)],
        "checkout slice branch for rebase",
    )
    .await?;
    if !checkout.status.success() {
        anyhow::bail!("git checkout {branch} failed: {}", output_stderr(&checkout));
    }

    // Try to fetch first; fall back to local ref
    let fetched = git_output(
        worktree_path,
        vec![
            OsString::from("fetch"),
            OsString::from("origin"),
            OsString::from(base_branch),
        ],
        "fetch base branch for rebase",
    )
    .await;
    let base_ref = if fetched.map(|o| o.status.success()).unwrap_or(false) {
        format!("origin/{base_branch}")
    } else {
        base_branch.to_string()
    };

    // Rebase onto the base branch
    let rebase = git_output(
        worktree_path,
        vec![
            OsString::from("rebase"),
            OsString::from("--"),
            OsString::from(&base_ref),
        ],
        "rebase slice branch onto base",
    )
    .await?;

    if rebase.status.success() {
        return Ok(());
    }

    // Rebase failed — abort and report
    let _ = git_output(
        worktree_path,
        vec![OsString::from("rebase"), OsString::from("--abort")],
        "abort failed rebase",
    )
    .await;

    anyhow::bail!(
        "git rebase {branch} onto {base_ref} failed: {}",
        output_stderr(&rebase)
    );
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
            review_artifacts: None,
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
