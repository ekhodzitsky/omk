use anyhow::Result;
use std::path::Path;

use crate::git::GitRepo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseOutcome {
    Clean,
    ConflictUnresolvable,
}

pub async fn attempt_auto_rebase(
    repo_dir: &Path,
    branch: &str,
    base_branch: &str,
) -> Result<RebaseOutcome> {
    validate_git_ref(branch)?;
    validate_git_ref(base_branch)?;

    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;

    repo.checkout(branch)
        .await
        .map_err(|e| anyhow::anyhow!("git checkout {branch} failed: {e}"))?;

    let fetch_ok = repo.fetch("origin").await.is_ok();
    let base_ref = if fetch_ok {
        format!("origin/{base_branch}")
    } else {
        base_branch.to_string()
    };

    if repo.rebase(&base_ref).await.is_err() {
        let _ = repo.rebase_abort().await;
        return Ok(RebaseOutcome::ConflictUnresolvable);
    }

    Ok(RebaseOutcome::Clean)
}

fn validate_git_ref(name: &str) -> Result<()> {
    if name.starts_with('-') {
        anyhow::bail!("invalid git ref name: cannot start with '-': {name}");
    }
    Ok(())
}
