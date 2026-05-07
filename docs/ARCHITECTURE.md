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

### `runtime/tmux.rs`
Thin wrapper around the `tmux` binary. Creates sessions, splits windows, sends keys, kills sessions.

### `runtime/bridge.rs`
Generates bash bridge scripts for worker panes. Workers poll `inbox.jsonl` and launch `kimi -p` for each task.

### `runtime/state.rs`
JSON state machines for Team, Autopilot, and Ralph modes. Persistent across process restarts.

### `runtime/worker.rs`
Worker specification and IPC helpers. `send_task()` appends JSONL to inbox; `read_results()` parses outbox.

## Skill System

Skills are markdown files with YAML frontmatter, discovered from:
1. `.omk/skills/` (project scope)
2. `~/.omk/skills/` (user scope)
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
2. OMK creates state directory `~/.omk/state/team/<name>/`
3. OMK creates tmux session with lead + 3 workers
4. Lead reads `skills/team/SKILL.md` and decomposes task into JSONL lines
5. Workers poll inboxes, execute with `kimi`, write results to outboxes
6. Lead monitors outboxes and synthesizes final answer
7. User runs `omk team status` to inspect progress
8. User runs `omk team shutdown` to clean up
