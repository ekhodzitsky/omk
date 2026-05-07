# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-05-07

### Added

- **XDG-compliant paths**: Config, state, data, and cache directories now follow the XDG Base Directory Specification. Legacy `~/.omk/` is still supported if it exists.
- **State schema versioning**: All state files (`TeamState`, `AutopilotState`, `RalphState`) now carry a `version` field with forward-migration support.
- **Metrics collection**: Telemetry for spawns, shutdowns, tasks, ask calls, autopilot/ralph runs. Persisted atomically in the state directory.
- **Atomic file writes**: `runtime/atomic.rs` writes to temp files and renames atomically to prevent corruption.
- **Retry logic**: Exponential backoff retry helper for resilient I/O and CLI calls.
- **Shell completions**: `omk completions <shell>` generates completions for bash, zsh, fish, elvish, and PowerShell.
- **Man page generation**: `omk man` outputs a roff man page.
- **Release CI**: GitHub Actions workflow builds multi-platform binaries (x86_64 Linux, x86_64 macOS, aarch64 macOS) on tag push.
- **Safe shell escaping**: Replaced all naive escaping with `shlex::try_quote` plus `validate_safe` input validation.

### Changed

- `install.sh` now installs shell completions and man pages alongside the binary.
- All hardcoded `~/.omk` paths migrated to centralized `runtime/config.rs` helpers.
- Zero compiler warnings in release build.

## [0.1.0] - 2026-05-06

### Added

- Initial release with Team Mode, Autopilot scaffold, Ralph scaffold, Ask scaffold, HUD scaffold, and MCP server scaffold.
- Tmux-native multi-agent orchestration with JSONL file-based IPC.
- Skill injection system compatible with Claude Code `SKILL.md` format.
- 23 unit and integration tests.
