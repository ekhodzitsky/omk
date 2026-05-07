# OMK Architecture

## Design Principles

1. **External Orchestrator**: OMK never forks or patches Kimi CLI. It coordinates `kimi` processes externally.
2. **File-Based IPC**: All inter-agent communication happens through JSONL inboxes/outboxes on disk.
3. **Tmux-Native**: Teams are first-class tmux sessions. You can attach, detach, and inspect them with standard tmux commands.
4. **Skill-Compatible**: Skills use the same `SKILL.md` format as Claude Code, ensuring portability.

## System Context

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   User      │────▶│    omk      │────▶│    tmux     │
│             │     │   (Rust)    │     │  (system)   │
└─────────────┘     └─────────────┘     └──────┬──────┘
                                                │
                       ┌────────────────────────┼────────────────────────┐
                       ▼                        ▼                        ▼
                ┌─────────────┐          ┌─────────────┐          ┌─────────────┐
                │  Lead Kimi  │          │ Worker Kimi │          │ Worker Kimi │
                │   (pane 0)  │          │  (pane 1)   │          │  (pane N)   │
                └──────┬──────┘          └──────┬──────┘          └──────┬──────┘
                       │                        │                        │
                       ▼                        ▼                        ▼
                ┌─────────────┐          ┌─────────────┐          ┌─────────────┐
                │  inbox.jsonl│          │  inbox.jsonl│          │  inbox.jsonl│
                │ outbox.jsonl│          │ outbox.jsonl│          │ outbox.jsonl│
                │heartbeat.json│         │heartbeat.json│         │heartbeat.json│
                └─────────────┘          └─────────────┘          └─────────────┘
```

## Runtime Modules

### `runtime/config.rs`
XDG Base Directory compliant path resolution. Config → `~/.config/omk/`, State → `~/.local/state/omk/`, Data → `~/.local/share/omk/`. Legacy `~/.omk/` fallback if it exists.

### `runtime/tmux.rs`
Thin wrapper around the `tmux` binary. Creates sessions, splits windows, sends keys, kills sessions.

### `runtime/bridge.rs`
Generates bash bridge scripts for worker panes. Workers poll `inbox.jsonl` and launch `kimi -p` for each task.

### `runtime/state.rs`
JSON state machines for Team, Autopilot, and Ralph modes. Persistent across process restarts. All states carry a `version` field for forward migration.

### `runtime/migrate.rs`
State schema migration. Reads `version` field, applies forward migrations, rejects future versions with a clear error message.

### `runtime/atomic.rs`
Atomic file writes via tempfile + `fs::rename`. Prevents readers from seeing partial writes even if the process crashes.

### `runtime/retry.rs`
Exponential backoff retry helper for resilient I/O and CLI calls.

### `runtime/metrics.rs`
Telemetry collection: spawns, shutdowns, tasks, ask calls, autopilot/ralph runs. Persisted as JSON in the state directory.

### `runtime/shell.rs`
Safe shell argument escaping via `shlex::quote` + input validation to prevent injection attacks.

### `runtime/worker.rs`
Worker specification and IPC helpers. `send_task()` appends JSONL to inbox; `read_results()` parses outbox.

## Skill System

Skills are markdown files with YAML frontmatter, discovered from:
1. `.omk/skills/` (project scope, legacy)
2. `~/.local/share/omk/skills/` (user scope, XDG)
3. `<omk binary dir>/skills/` (bundled)

The lead prompt can inject a bundled skill directly into the orchestration context.

## MCP Integration (Future)

OMK will expose tools via Model Context Protocol:
- `omk_team` — spawn teams
- `omk_status` — query state
- `omk_shutdown` — terminate sessions

This enables Cursor, Claude Desktop, and other MCP clients to orchestrate Kimi agents.

## Data Flow

1. User runs `omk team spawn 3:coder "task"`
2. OMK creates state directory `~/.local/state/omk/team/<name>/` (XDG) or `~/.omk/state/team/<name>/` (legacy)
3. OMK creates tmux session with lead + 3 workers
4. Lead reads `skills/team/SKILL.md` and decomposes task into JSONL lines
5. Workers poll inboxes, execute with `kimi`, write results to outboxes
6. Lead monitors outboxes and synthesizes final answer
7. User runs `omk team status` to inspect progress
8. User runs `omk team shutdown` to clean up
