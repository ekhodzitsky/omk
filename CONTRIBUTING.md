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

PRs use a macOS gate for `cargo fmt --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, feature checks, and
`cargo test`. The Ubuntu build job is temporarily disabled (no-op) while the
macOS runtime stabilises; cross-platform CI will be re-enabled in a future
milestone. Heavy validation stays on protected branches, scheduled runs, and
releases: full macOS build/test runs after merge, docs build outside the PR
fast path, coverage uploads from push/scheduled/manual runs, and release
artifacts are produced only by the release workflow.

External trackers (GitHub Issues, Linear, …) are optional and never a
prerequisite for building, testing, or reviewing the project. The canonical
handoff surface is the branch/worktree plus the PR body. Agents must not
silently initialize a tracker or make tracker state a runtime dependency.

## Verification Wall

Before opening a PR for code changes, run:

```bash
cargo fmt --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
cargo deny check advisories licenses sources --all-features
```

For documentation-only changes, at minimum run `cargo fmt --check` and
`git diff --check`; run broader gates when docs describe generated command
output, public API, or release behavior.

## Project Structure

```
src/
  analysis/   # tree-sitter based code analysis module
  bin/        # auxiliary binary crate directory (contains validate-contracts.rs)
  cli/        # Clap subcommands (setup, doctor, config, team, ask, autopilot, ralph, ultrawork, hud, kimi, goal, run, proof, backup, skill, logs, man, update)
  runtime/    # Core orchestration logic
    config.rs   # XDG path resolution + config.toml parsing
    goal/       # Goal controller, task graph, planner, decompose, scaffold
    team/       # Scheduler-backed team run, claim store, ownership map
    scheduler/  # Task dispatch, leases, retries
    gates/      # Verification gate runner
    events/     # Event log writing and replay
    proof/      # Proof/failure artifact generation
    worker.rs   # Worker spec + JSONL IPC
    state.rs    # JSON state machines
    atomic.rs   # Atomic file writes (tempfile + rename)
    metrics.rs  # Telemetry collection (JSON)
    migrate.rs  # State schema versioning + forward migrations
    retry.rs    # Exponential backoff retry helper
    shell.rs    # Safe shell escaping (shlex + validation)
  wire/       # Kimi Wire protocol client and parser
  agents/     # Agent runtime and role packs
  skills/     # Skill parser, discovery, injection
  vis/        # HUD / TUI / web dashboard
  mcp/        # MCP server
  marketplace/# Marketplace registry
  cost/       # Token/cost tracking
  notifications/ # Webhook and notification dispatch
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
