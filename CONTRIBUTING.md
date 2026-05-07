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
  cli/        # Clap subcommands
  runtime/    # Core orchestration logic
  skills/     # Skill parser, discovery, injection
  vis/        # HUD / TUI
  mcp/        # MCP server
```

## Conventional Commits

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation only
- `test:` adding or correcting tests
- `refactor:` code change that neither fixes a bug nor adds a feature
- `chore:` build process or auxiliary tool changes

## Versioning

We follow [SemVer](https://semver.org/). Update `VERSION` and `CHANGELOG.md` for releases.

## Code Style

- `cargo fmt` for formatting
- `cargo clippy -- -D warnings` for linting
- `anyhow::Result` for error handling
- `tracing` for logs

## Getting Help

Open an issue or discussion on GitHub.
