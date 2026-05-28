use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur when interacting with git repositories.
#[derive(Debug, Error, Clone)]
pub enum GitError {
    #[error("not a git repository: {0}")]
    NotARepo(PathBuf),

    #[error("git not found in path")]
    GitNotFound,

    #[error("command failed: {command} - exit {exit_code}, stderr: {stderr}")]
    CommandFailed {
        command: String,
        exit_code: i32,
        stderr: String,
        stdout: String,
    },

    #[error("command timed out after {0:?}: {1}")]
    Timeout(Duration, String),

    #[error("repository is not clean: {0}")]
    Dirty(String),

    #[error("branch already exists: {0}")]
    BranchExists(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("worktree already exists: {0}")]
    WorktreeExists(String),

    #[error("merge conflicts detected")]
    MergeConflicts(Vec<String>),

    #[error("io error: {0}")]
    Io(String),

    #[error("parse error: {0}")]
    Parse(String),
}

impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        GitError::Io(e.to_string())
    }
}
