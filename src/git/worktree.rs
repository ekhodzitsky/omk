use crate::git::error::GitError;
use crate::git::repo::GitRepo;
use std::path::{Path, PathBuf};

/// A typed handle to a git worktree.
#[derive(Debug, Clone)]
pub struct GitWorktree {
    pub(crate) path: PathBuf,
    pub(crate) branch: String,
}

impl GitWorktree {
    /// Path to the worktree directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Branch tracked by this worktree.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Execute a closure with a [`GitRepo`] opened at this worktree's path.
    #[allow(dead_code)]
    pub async fn repo<F, Fut, R>(&self, f: F) -> Result<R, GitError>
    where
        F: FnOnce(GitRepo) -> Fut,
        Fut: std::future::Future<Output = Result<R, GitError>>,
    {
        let repo = GitRepo::open(&self.path)?;
        f(repo).await
    }
}
