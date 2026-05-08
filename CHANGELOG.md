# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] - 2026-05-08

### Added

- **Integration tests**: 25 CLI smoke tests with `assert_cmd` covering all subcommands.
- **Structured errors**: `OmkError` enum with `thiserror`, status codes, categories, and JSON serialization for MCP/web.
- **Graceful shutdown**: Axum server handles SIGINT/SIGTERM with `tokio::signal`.
- **Deep health checks**: `/api/health` now checks tmux, kimi, and disk status.
- **Prometheus metrics**: `/metrics` endpoint serves `omk_total_spawns_total`, `omk_total_shutdowns_total`, etc.
- **Config validation**: Registry URLs must be http/https or file paths; `kimi_binary` must exist.

### Changed

- MCP tool errors now return structured JSON with `error`, `code`, and `category` fields.
- Zero warnings across lib, bin, tests, and doc-tests.

## [0.2.1] - 2026-05-08

### Added

- **External marketplace registries**: `omk marketplace add-registry/list-registries/remove-registry` with JSON registry format support (HTTP/HTTPS and local file paths).
- **Team management**: `omk team list`, `omk team attach`, `omk team broadcast`, `omk team rename`.
- **State visibility**: `omk state list` shows all teams, autopilots, and Ralph sessions.
- **Skill inspection**: `omk skill show` and `omk skill search` for installed skills.
- **Configuration**: `omk config set` for modifying config values at runtime.
- **Backup pruning**: `omk backup prune --keep N` removes old backups.
- **Artifact cleanup**: `omk cleanup --artifacts` removes old ask artifacts and log files.
- **Logging**: `omk logs` with `-n` (lines) and `-f` (follow) flags.
- **Version info**: `omk version` shows version, repository, and Rust compiler version.
- **Update check**: `omk update --check` verifies latest release without installing.
- **Marketplace info**: `omk marketplace info <skill>` shows detailed skill metadata.
- **Web dashboard**: Added `/api/autopilots` and `/api/ralphs` endpoints with dashboard cards.
- **Doctor registry check**: `omk doctor` validates all configured marketplace registries.
- **Documentation**: `docs/TUTORIAL.md`, `docs/API.md`, `docs/TROUBLESHOOTING.md`, `docs/REGISTRY.md`, and `examples/registry.json`.
- **Community**: Issue template chooser, stale issue bot, CODEOWNERS, FUNDING.yml.

### Changed

- Zero clippy warnings across all targets (lib, bin, tests).
- Updated feature status in README from "scaffolded" to "ready" for all modes.

## [0.2.0] - 2026-05-08

### Added

- **Autopilot**: Full 6-phase implementation with resume (`--resume`), YOLO mode (`--yolo`), visual progress reporting, multi-language QA (Rust, Node, Python, Go), and phase execution logging.
- **Ralph**: Full persistent loop with resume, YOLO mode, visual progress reporting, and escalation after 3 failures.
- **Ask**: Provider selection (`--providers`), timeout control (`--timeout`), synthesis disable (`--no-synthesis`).
- **Web dashboard**: `omk hud --web` serves axum HTTP API for teams/metrics/health.
- **Skill management**: `omk skill install/list/remove` for git-based skill installation.
- **State export/import**: `omk state export/import` for JSON-based state migration.
- **Backup/restore**: `omk backup create/list/restore` with tar.gz compression.
- **MCP tools**: Real CLI delegation for `omk_team_spawn`, `omk_team_status`, `omk_team_shutdown`, `omk_doctor`.
- **Code coverage**: `cargo-tarpaulin` + Codecov integration.

### Changed

- All runtime modules now use `#[allow(dead_code)]` to suppress scaffold warnings.
- CI builds with `--features server`.

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
