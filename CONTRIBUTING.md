# Contributing to oh-my-kimi

Thank you for your interest in contributing! We follow spec-driven development and test-driven development (TDD).

## Development Workflow

`master` / `main` are read-only. All development goes through an isolated
worktree or branch and a pull request.

1. **Pick the slice**: define the task/subgoal, owner, expected write scope,
   dependencies, and verification gates before editing.
2. **Create a branch/worktree**: use `agent/<task-slug>`,
   `codex/<task-slug>`, `kimi/<task-slug>`, or `claude/<task-slug>`.
3. **Read the spec**: check `SPEC.md`, `ROADMAP.md`, `TODO.md`, and relevant
   docs before implementing.
4. **Write tests first**: add tests in `tests/` or inline `#[cfg(test)]`
   modules for behavior changes.
5. **Implement inside the declared write scope**: if another agent owns the same
   files, serialize the work or use an integrator PR instead of racing the edit.
6. **Update docs**: if behavior changes, update `README.md`, `SPEC.md`,
   `CHANGELOG.md`, and focused docs in the same PR.
7. **Run checks**: use the verification wall below.
8. **Open a PR**: include task/scope, owner, risks, verification evidence, and
   known gaps.

### Pull Request Rule

Direct pushes to `master` / `main` are not part of the development workflow.
Use GitHub branch protection to require PRs, green CI, and review before merge.
PR branches should be small, reviewable, and tied to one task or one explicitly
named group of dependent tasks.

### CI Policy

PRs use a fast Ubuntu gate for `cargo fmt -- --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, feature checks, and
`cargo test`, plus a macOS compatibility smoke check. Heavy validation stays on
protected branches, scheduled runs, and releases: full macOS build/test runs
after merge, docs build outside the PR fast path, coverage uploads from
push/scheduled/manual runs, and release artifacts are produced only by the
release workflow.

External trackers (Beads, GitHub Issues, Linear, …) are optional and never a
prerequisite for building, testing, or reviewing the project. The canonical
handoff surface is the branch/worktree plus the PR body.

## Verification Wall

Before opening a PR for code changes, run:

```bash
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
cargo deny --all-features check advisories licenses
```

For documentation-only changes, at minimum run `cargo fmt -- --check` and
`git diff --check`; run broader gates when docs describe generated command
output, public API, or release behavior.

## Project Structure

```
src/
  cli/        # Clap subcommands (team, ask, autopilot, ralph, hud, doctor, cleanup, config)
  runtime/    # Core orchestration logic
    atomic.rs   # Atomic file writes (tempfile + rename)
    config.rs   # XDG path resolution + config.toml parsing
    metrics.rs  # Telemetry collection (JSON)
    migrate.rs  # State schema versioning + forward migrations
    retry.rs    # Exponential backoff retry helper
    shell.rs    # Safe shell escaping (shlex + validation)
    state.rs    # JSON state machines (Team, Autopilot, Ralph)
    worker.rs   # Worker spec + JSONL IPC
  skills/     # Skill parser, discovery, injection
  vis/        # HUD / TUI (scaffolded)
  mcp/        # MCP server (scaffolded)
```

## Commit Messages

Use the Lore protocol from `AGENTS.md`: an intent-first subject line plus
useful git trailers such as `Constraint:`, `Rejected:`, `Confidence:`,
`Scope-risk:`, `Directive:`, `Tested:`, and `Not-tested:`. Commit messages are
part of the project memory for future agents.

## Versioning

We follow [SemVer](https://semver.org/). Update `Cargo.toml` version and `CHANGELOG.md` for releases.

## Code Style

- `cargo fmt` for formatting
- `cargo clippy -- -D warnings` for linting (zero warnings policy)
- `anyhow::Result` for error handling
- `tracing` for logs
- `shlex::try_quote` + `validate_safe()` for all shell escaping
- `atomic_write()` for all state file writes
- XDG paths via `runtime::config` helpers (no hardcoded `~/.omk`)

## Getting Help

Open an issue or discussion on GitHub.
