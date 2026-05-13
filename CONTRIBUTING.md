# Contributing to oh-my-kimi

Thank you for your interest in contributing! We follow spec-driven development and test-driven development (TDD).

## Development Workflow

`master` / `main` are read-only. All development goes through a bead-scoped
branch and a pull request.

1. **Find or create the bead**: use `bd ready --json`, `bd show <id> --long`,
   or `bd create ...` for new work.
2. **Claim before editing**: `bd update <id> --claim --json`.
3. **Create a branch**: use `agent/<bead-id>-<slug>`,
   `codex/<bead-id>-<slug>`, `kimi/<bead-id>-<slug>`, or
   `claude/<bead-id>-<slug>`.
4. **Read the spec**: check `SPEC.md`, `ROADMAP.md`, `TODO.md`, and relevant
   docs before implementing.
5. **Write tests first**: add tests in `tests/` or inline `#[cfg(test)]`
   modules for behavior changes.
6. **Implement inside the claimed write scope**: if another agent owns the same
   files, create a dependency or integrator bead instead of racing the edit.
7. **Update docs**: if behavior changes, update `README.md`, `SPEC.md`,
   `CHANGELOG.md`, and focused docs in the same PR.
8. **Run checks**: use the verification wall below.
9. **Open a PR**: include bead id, write scope, risks, and verification
   evidence. Close the bead only after the PR is merged or explicitly abandoned.

### Agent Beads Protocol

Use Beads as the project control plane for concurrent agents:

```bash
bd ready --json
bd show <id> --long
bd update <id> --claim --json
git switch -c agent/<id>-<slug>
```

If `bd ready` reports that no database exists, do not let an agent initialize
one silently. A maintainer should choose the storage/sync mode first, for
example project-local Beads, a Dolt remote, or shared-server mode. The current
project bootstrap command avoids overwriting repo-maintained agent docs/hooks:

```bash
bd init --non-interactive --role maintainer --prefix omk --skip-agents --skip-hooks
```

During work, add important context to the bead so another agent can resume after
compaction or handoff. If a blocker appears, mark it in Beads and create a
dependent bead rather than leaving hidden state in a chat transcript.

### Pull Request Rule

Direct pushes to `master` / `main` are not part of the development workflow.
Use GitHub branch protection to require PRs, green CI, and review before merge.
PR branches should be small, reviewable, and tied to one bead or one explicitly
named group of dependent beads.

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
