use std::path::Path;

use crate::git::GitRepo;
use anyhow::Result;

pub(crate) async fn resolve_base_branch(repo_dir: &Path) -> Option<String> {
    let repo = GitRepo::open(repo_dir).ok()?;

    // First try to detect from origin/HEAD (works when remote is configured).
    if let Ok(branch) = repo.default_branch().await {
        return Some(branch);
    }

    // Fall back to checking for local main/master branches.
    for branch in ["main", "master"] {
        if repo.branch_exists(branch).await.ok()? {
            return Some(branch.to_string());
        }
    }
    None
}

pub(crate) async fn create_integrator_branch(
    repo_dir: &Path,
    integrator_branch: &str,
    base_branch: &str,
) -> Result<()> {
    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    repo.branch_create(integrator_branch, Some(base_branch))
        .await
        .map_err(|e| anyhow::anyhow!("git checkout -b failed: {e}"))?;
    repo.checkout(integrator_branch)
        .await
        .map_err(|e| anyhow::anyhow!("git checkout failed: {e}"))?;
    Ok(())
}

pub(crate) async fn merge_tree_is_clean(
    repo_dir: &Path,
    branch: &str,
    integrator_branch: &str,
) -> Result<()> {
    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    let result = repo
        .merge_tree(integrator_branch, branch)
        .await
        .map_err(|e| anyhow::anyhow!("git merge-tree failed: {e}"))?;
    if result.has_conflicts {
        anyhow::bail!("merge-tree predicts conflicts between {integrator_branch} and {branch}");
    }
    Ok(())
}

pub(crate) async fn merge_branch_into_integrator(
    repo_dir: &Path,
    branch: &str,
    integrator_branch: &str,
) -> Result<()> {
    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;

    repo.checkout(integrator_branch)
        .await
        .map_err(|e| anyhow::anyhow!("git checkout integrator failed: {e}"))?;

    let merge_err = match repo.merge(branch, true).await {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    repo.checkout(branch)
        .await
        .map_err(|e| anyhow::anyhow!("git merge failed and rebase checkout failed: {e}"))?;

    if let Err(_e) = repo.rebase(integrator_branch).await {
        let _ = repo.rebase_abort().await;
        let _ = repo.checkout(integrator_branch).await;
        anyhow::bail!("git merge failed and auto-rebase failed: {merge_err}");
    }

    repo.push_force("origin", branch)
        .await
        .map_err(|e| anyhow::anyhow!("git merge failed and rebase push failed: {e}"))?;

    repo.checkout(integrator_branch)
        .await
        .map_err(|e| anyhow::anyhow!("git checkout integrator failed after rebase: {e}"))?;

    repo.merge(branch, true)
        .await
        .map_err(|e| anyhow::anyhow!("git merge failed even after auto-rebase: {e}"))?;
    Ok(())
}

pub(crate) async fn push_branch(repo_dir: &Path, branch: &str) -> Result<()> {
    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    repo.push_force("origin", branch)
        .await
        .map_err(|e| anyhow::anyhow!("git push failed: {e}"))?;
    Ok(())
}
