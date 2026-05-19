//! Typed git operations with timeout, retry, and porcelain parsing.
//!
//! This module is a self-contained wrapper over the git CLI. It knows about
//! commands, timeouts, retries, output parsing, and error recovery. It does
//! **not** know about OMK-specific worktree semantics, goal lifecycle, or
//! slice execution.

mod branch;
mod command;
mod error;
mod parse;
mod repo;
mod types;
mod worktree;

#[cfg(test)]
mod tests;

pub use branch::GitBranch;
pub use error::GitError;
pub use repo::GitRepo;
pub use types::{GitLogEntry, GitMergeResult, GitRemote, GitStatus};
pub use worktree::GitWorktree;
