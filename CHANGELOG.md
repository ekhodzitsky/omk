# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Upstream Kimi docs tracking**: added `docs/KIMI_UPSTREAM.md` to record the official Kimi docs URLs we re-check before Kimi integration releases, plus the last checked date and protocol note.
- **Run timeline filtering and hardening**: `omk run show` now supports worker/task/kind filters plus JSON output, Wire message loops explicitly skip unknown methods/events and error unknown request types, rollback reports corrupt backup restore failures without stopping unrelated cleanup, and README records the exact local verification commands.
- **Verification-First Completion**: `VerificationGate` config model with presets for Rust, Node, Python, and Go. Autopilot QA phase and Ralph verify loops now run structured gates (format, lint, tests, etc.) and block completion until all required gates pass. `DoneContract` persists run evidence (gates, changed files, known gaps) as JSON.
- **Ultrawork mode**: `omk ultrawork` (alias `uw`) runs multiple kimi prompts in parallel with configurable concurrency. Supports task args, `--file` (one per line), `--files` glob with `--prompt` template, and `--output` JSON. Includes cost tracking, AGENTS.md injection, and webhook notifications.
- **Cost tracking wired across all modes**: `team`, `autopilot`, and `ralph` now record session costs to `costs.json` with heuristic estimates shown at start and actual duration-based estimates at completion.
- **AGENTS.md injection in autopilot & ralph**: All prompts (expansion, planning, execution, architect review, security review, ralph implementation, ralph escalation) now include project agent context when `.omk/AGENTS.md` is present.
- **Kimi-native asset management**: `omk kimi sync`, `install`, `doctor`, and `rollback` manage `.kimi/agents/`, `.kimi/hooks/`, and `.kimi/skills/` with an `omk-manifest.json` that records every file OMK owns. Drift detection identifies missing or modified managed files.
- **Runtime Scheduler core**: `Task`, `ClaimStore`, `OwnershipMap`, and `RunManifest` types for task lifecycle, atomic claims with leases, stale-lease recovery, retry logic, cascade cancellation, and file ownership conflict detection.
- **Event Log Core**: Typed `Event` envelope with `RunId`, `EventId`, `WorkerId`, `TaskId`, and `GateId` newtypes. Append-only JSONL `EventWriter` and tolerant `EventReader` that skips malformed lines. `EventBuilder` helpers for common event patterns. Events are written to `<team_state_dir>/events.jsonl` by `omk team spawn` and `shutdown`.
- **Watchdog**: `omk team health <name>` runs health checks on team workers, detecting missing heartbeats, stale heartbeats, and dead tmux sessions. Issues are recorded as `worker_stalled` events in `events.jsonl`.
- **Team spawn instrumentation**: `run_started`, `worker_started`, `run_failed`, and `manual_interrupt` events are emitted during `omk team spawn` and `omk team shutdown`.
- **Manifest checksums & drift detection**: FNV-1a 64-bit checksums are computed for every managed Kimi asset and stored in `omk-manifest.json`. `omk kimi doctor` detects both missing files and content drift (checksum mismatches).
- **Backup before overwrite**: `omk kimi sync` and `install` create `.omk-backup-{timestamp}` copies before overwriting existing non-identical files.
- **Auto-cleanup**: `omk team cleanup` removes old team state directories with `--older-than <days>`, `--dry-run`, and `--all` flags. `omk cleanup --teams` provides the same functionality from the top-level cleanup command.
- **Mock Kimi fixture**: `mock-kimi` binary (`tests/fixtures/mock_kimi.rs`) simulates the Kimi CLI for CI testing. Supports `--version`, `--help`, `-p <prompt_file>`, and keyword-triggered success/error modes.
- **`omk team run` scaffold**: New `TeamRunner` orchestrates scheduler-based team execution using `ClaimStore`, `OwnershipMap`, and `RunManifest`. Dispatches tasks to worker inboxes, polls outboxes for completion/failure, and emits `TaskClaimed`, `TaskStarted`, `TaskCompleted`, `TaskFailed`, `RetryScheduled` events.
- **Event-backed HUD**: `omk hud <team_name>` displays live team status with `--once` and `--json` modes. `EventStream` provides incremental event reading; `HudState` renders worker health, task summary, and runtime.
- **Rate-limit backoff**: `retry.rs` detects HTTP 429 / rate-limit responses in stderr and applies a fixed 30s delay instead of exponential backoff. `is_rate_limited()` helper and `run_command_with_retry()` shell utility added.
- **Watchdog recovery**: `omk team health <name> --recover` attempts to restart dead workers by re-spawning their bridge scripts. Recovery results are recorded as `WorkerRecovered` events.
- **Dry-run support**: `omk kimi sync --dry-run`, `omk kimi install --dry-run`, and `omk kimi rollback --dry-run` preview changes without modifying the filesystem.
- **Manifest schema version**: `MANIFEST_SCHEMA_VERSION = 1` constant added; `AssetManifest::load()` validates version compatibility.
- **Interactive HUD TUI**: `omk hud --tui` (requires `--features tui`) renders a real-time ratatui dashboard with worker health table, task status panel, event stream, and color-coded status indicators. Supports `q` to quit and `r` to refresh.
- **Proof golden tests**: `tests/proof_golden_test.rs` provides deterministic fixture-based proof generation scenarios — happy path, failure path, empty run, and direct gate results. `tests/fixture_runner.rs` is a reusable test helper for emitting events and generating proofs.
- **Role packs**: `omk team roles` lists 5 curated role packs (architect, executor, verifier, reviewer, integrator) with descriptions, suggested worker counts, and default skills. `omk team spawn --role-pack <id>` selects a pack directly.
- **Bridge task protocol**: Workers now read tasks from inbox, execute them via `kimi -p <prompt>`, and write structured results (`WorkerResult` JSON) to outbox. `MOCK_KIMI` env var selects the mock binary for testing. Bridge script includes heartbeat loop and json_build_result helper.
- **Wire Protocol types**: Complete Rust type definitions for Kimi Code CLI wire mode (`src/wire/protocol.rs`) — JSON-RPC 2.0 messages, all 17 event types, 4 request types, display blocks, error codes, initialization handshake.
- **Wire client**: `WireClient` (`src/wire/client.rs`) spawns `kimi --wire`, sends requests, reads events/requests/responses, handles approval flow. Supports `initialize`, `prompt`, `cancel`, and message loop processing.
- **Wire protocol tests**: 16 integration tests covering JSON-RPC serialization, event/request/response parsing, content parts, tool calls, display blocks, status updates, and wire message union type discrimination.
- **AGENTS.md wire reference**: `.omk/AGENTS.md` expanded with comprehensive wire protocol documentation including initialization handshake, prompt flow, event/request tables, error codes, and Kimi Agent (Rust) notes.
- **Autopilot & Ralph notifications**: Webhook notifications (Discord, Slack, Telegram) fire on autopilot complete/failed and ralph complete events.

## [0.2.4] - 2026-05-08

### Added

- **AGENTS.md parser & runtime**: Discovers `.omk/AGENTS.md`, parses YAML frontmatter + markdown body, and injects relevant agent context into prompts.
- **7 specialized agent skills**: `architect`, `frontend`, `backend`, `security`, `devops`, `data`, `qa` with trigger-based matching.
- **Notification webhooks**: Discord, Slack, Telegram support for team spawn/shutdown events.
- **Magic keywords**: `t` (team), `ap` (autopilot), `r` (ralph), `s` (skill), `m` (marketplace) aliases.
- **`omk doctor`**: Validates AGENTS.md syntax and project structure.

## [0.2.3] - 2026-05-08

### Added

- **`omk team export/import`**: JSON roundtrip for team state.

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

## [0.2.3] - 2026-05-08

### Added

- **Team export/import**: `omk team export <name>` and `omk team import <file>` for JSON-based team state migration.
- Additional integration tests for team spawn validation and export/import roundtrip.

### Changed

- Version bumped to 0.2.3 across Cargo.toml, dashboard, and tests.
