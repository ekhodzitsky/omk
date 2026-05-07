# OMK Specification

## Overview

OMK (oh-my-kimi) is a multi-agent orchestration layer for the Kimi CLI. It runs *outside* Kimi CLI as an external process orchestrator, spawning and coordinating multiple `kimi` instances via tmux and file-based IPC.

## Team Mode

### Command

```
omk team <N:ROLE> [OPTIONS] <TASK...>
```

### Spec: Spawn

1. Parse `N:ROLE` (e.g., `3:coder`).
2. Generate a team name or accept `--name`.
3. Ensure tmux is installed.
4. Create state directory: `~/.omk/state/team/<name>/`.
5. Write `team-state.json` with initial state (`phase: Planning`).
6. Create tmux session `omk-team-<name>` with window `lead`.
7. In pane 0, spawn lead `kimi` process with orchestration prompt.
8. For each worker `i` in `0..N`:
   a. Create worker directory `workers/worker-<i>/`.
   b. Write `worker-spec.json` (inbox, outbox, heartbeat paths).
   c. Split tmux window.
   d. Rename pane.
   e. Spawn bridge script that polls inbox and launches `kimi -p` per task.
9. Select tmux layout `tiled`.
10. Print team summary with attach instructions.

### Spec: Status

```
omk team status <NAME>
```

1. Resolve state directory: `~/.omk/state/team/<name>/`.
2. Load `team-state.json`.
3. For each worker:
   a. Read `worker-spec.json`.
   b. Check `heartbeat.json` for liveness.
   c. Count tasks in inbox and outbox.
4. Print table:
   - Team name, task, phase, created_at
   - Workers: name, role, status (ready|running|dead), inbox_count, outbox_count
   - Tasks: list with status
5. If tmux session does not exist, warn and mark as `orphaned`.

### Spec: Shutdown

```
omk team shutdown <NAME> [--force]
```

1. Resolve state directory.
2. Load `team-state.json`.
3. If tmux session `omk-team-<name>` exists:
   a. Send `Ctrl-C` to all panes (graceful interrupt).
   b. Wait 2s.
   c. Kill tmux session.
4. If `--force`, skip graceful steps.
5. Update `team-state.json` with `phase: Shutdown`.
6. Optionally archive state to `~/.omk/state/team/.archive/<name>-<timestamp>/`.
7. Print confirmation.

## State Schema

### TeamState

```json
{
  "name": "string",
  "task": "string",
  "created_at": "2026-05-07T14:50:00Z",
  "worker_count": 3,
  "worker_role": "coder",
  "phase": "Planning|Executing|Verifying|Fixing|Complete|Failed|Shutdown",
  "tasks": [
    {
      "id": "uuid",
      "description": "string",
      "assigned_to": "worker-0",
      "status": "Pending|InProgress|Done|Failed",
      "created_at": "2026-05-07T14:50:00Z",
      "completed_at": null
    }
  ],
  "state_dir": "/home/user/.omk/state/team/name"
}
```

### WorkerSpec

```json
{
  "name": "worker-0",
  "role": "coder",
  "inbox": "/home/user/.omk/state/team/name/workers/worker-0/inbox.jsonl",
  "outbox": "/home/user/.omk/state/team/name/workers/worker-0/outbox.jsonl",
  "heartbeat": "/home/user/.omk/state/team/name/workers/worker-0/heartbeat.json"
}
```

## Bridge Protocol

### Inbox Line Format (JSONL)

```json
{"id":"<uuid>","task":"<description>","acceptance_criteria":["..."],"context":"<optional>"}
```

### Outbox Line Format (JSONL)

```json
{"task_id":"<uuid>","status":"success|partial|failed","summary":"...","artifacts":["paths"],"elapsed_secs":42}
```

### Heartbeat Format

```json
{"status":"ready|alive|dead","name":"worker-0","ts":"2026-05-07T14:50:00Z"}
```

## Lead Prompt Contract

The lead agent MUST:
1. Decompose the task into parallel subtasks with acceptance criteria.
2. Write each subtask as a single JSONL line to the appropriate worker inbox.
3. Monitor outbox files for results.
4. Reassign failed tasks (max 2 retries per worker).
5. Synthesize final answer when all tasks complete.
6. Report progress via `TaskList` tool if running in Ralph mode.

## Test Strategy

- **Unit tests**: skill parser, state serialization, spec parsing, tmux command generation.
- **Integration tests**: spawn a mock team (using `echo` instead of `kimi`), verify JSONL flow, status output.
- **E2E tests**: require real `kimi` CLI and tmux — run manually or in CI with mocks.
