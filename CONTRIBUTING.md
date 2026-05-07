# Contributing to oh-my-kimi

Thank you for your interest in contributing! We follow spec-driven development and test-driven development (TDD).

## Development Workflow

1. **Read the spec**: Check `SPEC.md` before implementing.
2. **Write tests first**: Add tests in `tests/` or inline `#[cfg(test)]` modules.
3. **Implement**: Make the tests pass.
4. **Update docs**: If you change behavior, update `README.md`, `SPEC.md`, and `CHANGELOG.md`.
5. **Run checks**: `make check` (fmt + clippy + test).

## Project Structure

```
src/
  cli/        # Clap subcommands (team, ask, autopilot, ralph, hud, doctor, cleanup, config)
  runtime/    # Core orchestration logic
    atomic.rs   # Atomic file writes (tempfile + rename)
    bridge.rs   # Worker bridge scripts for tmux panes
    config.rs   # XDG path resolution + config.toml parsing
    metrics.rs  # Telemetry collection (JSON)
    migrate.rs  # State schema versioning + forward migrations
    retry.rs    # Exponential backoff retry helper
    shell.rs    # Safe shell escaping (shlex + validation)
    state.rs    # JSON state machines (Team, Autopilot, Ralph)
    tmux.rs     # tmux binary wrapper
    worker.rs   # Worker spec + JSONL IPC
  skills/     # Skill parser, discovery, injection
  vis/        # HUD / TUI (scaffolded)
  mcp/        # MCP server (scaffolded)
```

## Conventional Commits

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation only
- `test:` adding or correcting tests
- `refactor:` code change that neither fixes a bug nor adds a feature
- `chore:` build process or auxiliary tool changes
- `ci:` CI/CD changes
- `config:` configuration or path resolution changes

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
