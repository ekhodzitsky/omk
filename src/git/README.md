# `src/git/` — Typed Git Operations

## Purpose

Self-contained typed wrapper over the git CLI for OMK and any other consumer.
Handles command execution, timeouts, retries, porcelain parsing, and structured
error recovery.

## Status

- [x] Core types and errors
- [x] Command wrapper with timeout/retry
- [x] Repository operations
- [x] Worktree operations
- [x] Branch helpers
- [x] Porcelain parsers with fixture tests
- [x] Integration tests

## Public API

### `GitRepo`

Open a repository and perform operations:

```rust
let repo = GitRepo::open("/path/to/repo")?;
repo.ensure_clean().await?;
let branch = repo.current_branch().await?;
let wt = repo.worktree_add("/tmp/wt", "feature/x").await?;
```

### `GitWorktree`

Typed worktree handle:

```rust
let wt = repo.worktree_add("/tmp/wt", "feature/x").await?;
assert_eq!(wt.branch(), "feature/x");
wt.repo(|r| async move { r.ensure_clean().await }).await?;
```

### `GitBranch`

Validated branch name:

```rust
let b = GitBranch::new("feature/x")?;
assert_eq!(b.name(), "feature/x");
```

### Errors

All operations return `Result<T, GitError>`:

- `NotARepo` — path has no `.git`
- `GitNotFound` — git binary not in PATH
- `CommandFailed` — non-zero exit code
- `Timeout` — command exceeded 30s
- `Dirty` — uncommitted changes
- `BranchExists` / `BranchNotFound`
- `WorktreeExists`
- `MergeConflicts`

## Dependencies

- `tokio` — async process spawning and timeouts
- `tracing` — structured logging
- `thiserror` — error definitions
- `which` — git binary discovery (already in workspace)

## File Map

| File        | Contents                                      |
|-------------|-----------------------------------------------|
| `mod.rs`    | Module exports                                |
| `repo.rs`   | `GitRepo` — open, status, branch, commit, …   |
| `worktree.rs` | `GitWorktree` — typed worktree handle       |
| `branch.rs` | `GitBranch` — validation helpers              |
| `command.rs` | `GitCommand` — internal timeout/retry wrapper |
| `parse.rs`  | Pure porcelain parsers + fixture tests        |
| `error.rs`  | `GitError` enum                               |
| `types.rs`  | `GitStatus`, `GitLogEntry`, `GitRemote`, etc. |
