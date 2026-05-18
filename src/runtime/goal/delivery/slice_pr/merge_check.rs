use anyhow::Result;
use std::ffi::OsString;
use std::path::Path;

use super::git::{git_output, git_worktree_has_changes, output_stderr, output_stdout};

/// Check whether the slice branch merges cleanly into the base branch.
/// Uses a temporary `git merge --no-commit --no-ff` so the working tree
/// is not permanently altered. Returns Ok if clean, Err if conflicts are predicted.
pub(super) async fn check_slice_branch_merge_clean(
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
                OsString::from(super::STASH_MESSAGE),
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
            OsString::from(super::DEFAULT_REMOTE),
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
