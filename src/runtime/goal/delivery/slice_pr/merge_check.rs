use anyhow::Result;
use std::path::Path;

use crate::git::GitRepo;

/// Check whether the slice branch merges cleanly into the base branch.
/// Uses a read-only merge-tree check so the working tree is not altered.
/// Returns Ok if clean, Err if conflicts are predicted.
pub(super) async fn check_slice_branch_merge_clean(
    worktree_path: &Path,
    branch: &str,
    base_branch: &str,
) -> Result<()> {
    let repo = GitRepo::open(worktree_path)
        .map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    let result = repo
        .merge_tree(base_branch, branch)
        .await
        .map_err(|e| anyhow::anyhow!("merge-tree check failed: {e}"))?;
    if result.has_conflicts {
        anyhow::bail!(
            "merge predicts conflicts in files: {}",
            result.conflict_files.join(", ")
        );
    }
    Ok(())
}
