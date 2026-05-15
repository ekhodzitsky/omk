---
schema_version: 1
module: vis
level: root
purpose: Render human-readable and machine-readable views of OMK runtime state.
status: pilot
surface:
  - name: EventStream
    kind: struct
    visibility: pub
    contract: Incremental, non-blocking tail-reader for a JSONL event file. Returns only new events since the last poll, handles file truncation gracefully, and skips malformed lines with a warning.
    proof:
      kind: integration-test
      target: tests/hud_test.rs::test_event_stream_poll_reads_incremental_events
      command: cargo test --test hud_test test_event_stream_poll_reads_incremental_events
  - name: render_goal_progress
    kind: fn
    visibility: pub
    contract: Produce a deterministic, structured text summary from a GoalProgressSnapshot. Output contains no chat-style prefixes ("assistant:", "user:").
    proof:
      kind: integration-test
      target: tests/goal_progress_test.rs::terminal_progress_render_is_structured_not_chat
      command: cargo test --test goal_progress_test terminal_progress_render_is_structured_not_chat
  - name: HudState
    kind: struct
    visibility: pub
    contract: Mutable snapshot of team run state (events, workers, tasks, gates, proof). Can be refreshed from an EventStream + Watchdog and rendered to text or JSON.
    proof:
      kind: integration-test
      target: tests/hud_test.rs::test_hud_state_refresh_and_render
      command: cargo test --test hud_test test_hud_state_refresh_and_render
  - name: TaskSummary
    kind: struct
    visibility: pub
    contract: Read-model counts of tasks by status (total, completed, running, pending, failed). Derived from TeamState or event stream fallback.
    proof:
      kind: unit-test
      target: src/vis/hud/render.rs::hud_state_render_text_expected_output
      command: cargo test hud_state_render_text_expected_output
  - name: WorkerDisplay
    kind: struct
    visibility: pub
    contract: Per-worker read-model combining health status, current task, retry count, heartbeat age, and latest gate status.
    proof:
      kind: integration-test
      target: tests/hud_test.rs::test_hud_worker_display_computation
      command: cargo test --test hud_test test_hud_worker_display_computation
  - name: strip_ansi
    kind: fn
    visibility: pub(crate)
    contract: Remove ANSI escape sequences (CSI, OSC, DCS, APC, PM, SOS) and bare control bytes from untrusted strings while preserving tabs and newlines.
    proof:
      kind: unit-test
      target: src/vis/hud/sanitize.rs
      command: cargo test strip_ansi
  - name: HudTui
    kind: struct
    visibility: pub
    contract: Interactive ratatui dashboard that polls events and redraws at 1 Hz. Terminal raw mode / alt-screen / mouse capture are restored on panic or early return via RawModeGuard.
    proof:
      kind: unit-test
      target: src/vis/hud_tui/mod.rs::hud_tui_draw_does_not_panic
      command: cargo test hud_tui_draw_does_not_panic
  - name: run_server
    kind: fn
    visibility: pub
    contract: Axum-based web dashboard serving an embedded HTML UI, REST API (/api/teams, /api/autopilots, /api/ralphs, /api/metrics, /api/health), and a /metrics Prometheus endpoint. Shuts down gracefully on Ctrl+C or SIGTERM.
    proof:
      kind: smoke
      target: src/vis/server/bootstrap.rs
      command: cargo check --features server
  - name: hud
    kind: module
    visibility: pub
    contract: Subsystem owning HUD state types, refresh logic, text/JSON renderers, and ANSI sanitizer.
    proof:
      kind: static-check
      target: src/vis/hud/mod.rs
      command: cargo check
  - name: hud_tui
    kind: module
    visibility: pub
    contract: Feature-gated (`tui`) subsystem providing the ratatui terminal dashboard.
    proof:
      kind: static-check
      target: src/vis/hud_tui/mod.rs
      command: cargo check --features tui
  - name: server
    kind: module
    visibility: pub
    contract: Feature-gated (`server`) subsystem providing the web dashboard and Prometheus metrics exporter.
    proof:
      kind: static-check
      target: src/vis/server/mod.rs
      command: cargo check --features server
dependencies:
  internal:
    - module: runtime::events
      scope: EventStream reads Event/RunId/EventKind; HudState consumes events for task tracking and gate/proof extraction.
      reason: vis is a read-only view layer over the runtime event log.
    - module: runtime::state
      scope: HudState::refresh loads TeamState for task summaries and start-time fallback.
      reason: Ground-truth for task counts and worker metadata.
    - module: runtime::watchdog
      scope: HudState::refresh calls Watchdog::check_team_read_only for WorkerHealth snapshots.
      reason: Health data is produced by the watchdog, consumed by the HUD.
    - module: runtime::config
      scope: server handlers read state_dir() and omk_state_dir() to locate JSON state files.
      reason: Server routes expose whatever state exists on disk.
    - module: runtime::goal
      scope: render_goal_progress accepts GoalProgressSnapshot.
      reason: Goal progress text is a human-readable rendering of goal runtime state.
  external:
    - name: tokio
      scope: async file I/O, intervals, signal handling, TCP listener
      reason: All vis I/O is async; server and TUI both need the runtime.
    - name: anyhow
      scope: error propagation across vis boundary
      reason: Vis functions report errors rather than panicking.
    - name: serde / serde_json
      scope: JSON serialization of HudState, event parsing, metrics normalization
      reason: Machine-readable output and event wire format.
    - name: chrono
      scope: timestamps in HudState, runtime duration formatting
      reason: Human-readable durations and event ordering.
    - name: tracing
      scope: warn! on malformed event lines
      reason: Observability for skipped data.
    - name: ratatui
      scope: TUI layout, widgets, styling (feature = "tui")
      reason: Terminal dashboard rendering.
    - name: crossterm
      scope: raw mode, alt screen, mouse capture, keyboard events (feature = "tui")
      reason: Terminal control for the TUI.
    - name: axum
      scope: HTTP router and handlers (feature = "server")
      reason: Web dashboard server.
    - name: tower
      scope: Axum service stack (feature = "server")
      reason: Required by axum for serving.
consumers:
  - path: src/cli/hud.rs
    uses:
      - EventStream::new
      - HudState::new
      - HudState::refresh
      - HudState::render_text
      - HudState::render_json
      - HudTui::new
      - HudTui::run
      - run_server
  - path: tests/hud_test.rs
    uses:
      - EventStream::new
      - EventStream::poll
      - HudState::new
      - HudState::refresh
      - HudState::render_text
      - HudState::render_json
      - HudState::worker_displays
  - path: tests/goal_progress_test.rs
    uses:
      - render_goal_progress
invariants:
  - id: event-stream-incremental
    rule: EventStream::poll returns only events appended since the previous poll, and resumes from the beginning if the file was truncated.
    proof:
      kind: integration-test
      target: tests/hud_test.rs::test_event_stream_poll_reads_incremental_events
      command: cargo test --test hud_test test_event_stream_poll_reads_incremental_events
  - id: ansi-sanitization
    rule: strip_ansi removes all ANSI escape sequences and bare control bytes while preserving tabs and newlines.
    proof:
      kind: unit-test
      target: src/vis/hud/sanitize.rs
      command: cargo test strip_ansi
  - id: tui-terminal-restore
    rule: HudTui::run restores terminal raw mode, alt screen, and mouse capture on panic or early return.
    proof:
      kind: unit-test
      target: src/vis/hud_tui/mod.rs::hud_tui_draw_does_not_panic
      command: cargo test hud_tui_draw_does_not_panic
  - id: no-panic-in-handlers
    rule: Server handlers never panic; I/O errors and parse failures are handled gracefully.
    proof:
      kind: static-check
      target: src/vis/server/handlers.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: hud-render-deterministic
    rule: HudState::render_text output is deterministic for identical state; it does not depend on time-of-call beyond pre-computed timestamps.
    proof:
      kind: unit-test
      target: src/vis/hud/render.rs::hud_state_render_text_expected_output
      command: cargo test hud_state_render_text_expected_output
  - id: goal-progress-structured
    rule: render_goal_progress output never contains chat-style prefixes and always includes the same sections in the same order.
    proof:
      kind: integration-test
      target: tests/goal_progress_test.rs::terminal_progress_render_is_structured_not_chat
      command: cargo test --test goal_progress_test terminal_progress_render_is_structured_not_chat
verification:
  pre_change:
    - cargo test --lib vis
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# vis

## Architecture

`vis` is the read-only presentation layer for OMK runtime state. It consumes event logs, team state, and watchdog health reports, then produces human-readable text, JSON, TUI dashboards, and web UIs.

```
┌─────────────────────────────────────────────┐
│  CLI (src/cli/hud.rs)                        │
│  ───────────────────                         │
│  --once  → text / JSON snapshot              │
│  --tui   → HudTui (ratatui)                  │
│  --web   → run_server (axum)                 │
└──────────────┬──────────────────────────────┘
               │
       ┌───────┴───────┐
       ▼               ▼
┌─────────────┐  ┌─────────────┐
│  event_stream │  │     hud     │
│  (tail JSONL) │  │  (state +   │
│               │  │   render)   │
└─────────────┘  └─────────────┘
       │               │
       └───────┬───────┘
               ▼
    ┌─────────────────────┐
    │ runtime::{events,   │
    │ state, watchdog}    │
    └─────────────────────┘
```

### Data flow

1. `EventStream` tails `events.jsonl` incrementally.
2. `HudState::refresh` merges new events, loads `TeamState`, and runs a read-only watchdog check.
3. `HudState::render_text` / `render_json` produce output for `--once` and the web API.
4. `HudTui` wraps the same refresh loop in a 1 Hz ratatui draw cycle with keyboard input.
5. `server` routes read state files directly from disk for the web dashboard, independent of the HUD refresh cycle.

### Feature gates

| Feature | Modules | External deps |
|---------|---------|---------------|
| `tui` (default) | `hud_tui` | `ratatui`, `crossterm` |
| `server` | `server` | `axum`, `tower` |

## Files

| File | Lines | Role |
|------|-------|------|
| `mod.rs` | 11 | Storefront: re-exports `event_stream`, `goal_progress`, `hud`, and feature-gated `hud_tui` / `server`. |
| `event_stream.rs` | 111 | `EventStream` struct: incremental async tail-reader for JSONL events with position tracking and truncation handling. |
| `goal_progress.rs` | 69 | `render_goal_progress`: deterministic text formatter for `GoalProgressSnapshot`. |
| `hud/mod.rs` | 7 | Re-exports `HudState`, `TaskSummary`, `WorkerDisplay` (pub) and `strip_ansi` (pub(crate)). |
| `hud/types.rs` | 59 | Data types: `HudState`, `TaskSummary`, `WorkerDisplay`. |
| `hud/state.rs` | 137 | `HudState::refresh` implementation: event polling, TeamState loading, task summary computation, watchdog health check. |
| `hud/render.rs` | 266 | `HudState::render_text`, `HudState::render_json`, `worker_displays`, and private gate/proof extractors. |
| `hud/sanitize.rs` | 117 | `strip_ansi`: ANSI escape sequence remover with control-byte stripping. |
| `hud_tui/mod.rs` | 191 | `HudTui` struct: event loop, keyboard handling, refresh orchestration, `RawModeGuard` integration. |
| `hud_tui/guard.rs` | 32 | `RawModeGuard`: RAII terminal restoration on Drop. |
| `hud_tui/render.rs` | 244 | ratatui `draw` implementation: header, workers table, tasks table, events list, footer. |
| `server/mod.rs` | 6 | Re-exports `run_server`. |
| `server/bootstrap.rs` | 27 | Axum router setup and `tokio::net::TcpListener` serve loop with graceful shutdown. |
| `server/handlers.rs` | 179 | API handlers: `/api/teams`, `/api/autopilots`, `/api/ralphs`, `/api/metrics`, `/api/health`, `/metrics` (Prometheus). |
| `server/html.rs` | 285 | Embedded dashboard HTML + JavaScript, served at `/`. |
| `server/signal.rs` | 32 | Ctrl+C and SIGTERM graceful shutdown signal handler. |
