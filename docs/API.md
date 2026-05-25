# OMK API Reference

## MCP Server (JSON-RPC 2.0 over stdio)

Start the MCP server:

```bash
omk mcp-server
```

### Tools

#### `omk_team_run`

Run a scheduler-backed Kimi team through the Wire runtime.

**Parameters:**

| Name | Type | Description |
| --- | --- | --- |
| `spec` | string | Worker spec, for example `3:executor`. |
| `task` | string | Task description. |
| `name` | string | Optional team name. |

**Response:**

```json
{
  "status": "completed",
  "stdout": "...",
  "stderr": "",
  "spec": "3:executor",
  "task": "refactor authentication"
}
```

#### `omk_team_status`

Get team status.

**Parameters:**

| Name | Type | Description |
| --- | --- | --- |
| `name` | string | Team name. |

**Response:**

```json
{
  "status": "ok",
  "team": "executor-a1b2",
  "stdout": "...",
  "stderr": ""
}
```

#### `omk_team_shutdown`

Shutdown or mark a team interrupted.

**Parameters:**

| Name | Type | Description |
| --- | --- | --- |
| `name` | string | Team name. |
| `force` | boolean | Force shutdown handling. Defaults to `false`. |

**Response:**

```json
{
  "status": "shutdown",
  "team": "executor-a1b2",
  "force": false,
  "stdout": "...",
  "stderr": ""
}
```

#### `omk_doctor`

Run environment diagnostics.

**Parameters:** none

**Response:**

```json
{
  "status": "healthy",
  "healthy": true,
  "stdout": "...",
  "stderr": ""
}
```

## Web Dashboard REST API

Base URL: `http://localhost:8080`

### `GET /api/health`

```json
{
  "status": "ok",
  "version": "<crate-version>",
  "checks": {
    "kimi": {
      "status": "ok"
    },
    "disk": {
      "status": "ok"
    }
  }
}
```

`version` matches the running `omk` crate version reported by `omk --version`.

### `GET /api/teams`

```json
{
  "teams": [
    {
      "version": 1,
      "name": "executor-a1b2",
      "task": "refactor authentication",
      "created_at": "2026-05-11T12:00:00Z",
      "worker_count": 3,
      "worker_role": "executor",
      "phase": "Executing",
      "tasks": [],
      "state_dir": "/home/user/.local/state/omk/team/executor-a1b2"
    }
  ]
}
```

### `GET /api/autopilots`

```json
{
  "autopilots": [
    {
      "version": 1,
      "task": "build REST API",
      "phase": "Execution",
      "plans_dir": "/home/user/.local/state/omk/autopilot/ap-xxx/plans",
      "created_at": "2026-05-11T12:00:00Z"
    }
  ]
}
```

### `GET /api/ralphs`

```json
{
  "ralphs": [
    {
      "version": 1,
      "task": "implement auth",
      "iteration": 3,
      "max_iterations": 10,
      "state_dir": "/home/user/.local/state/omk/ralph/auth"
    }
  ]
}
```

### `GET /api/metrics`

Aggregated runtime counters.

```json
{
  "metrics": {
    "version": 1,
    "created_at": "2026-05-11T12:00:00Z",
    "updated_at": "2026-05-11T12:10:00Z",
    "total_team_runs": 12,
    "total_spawns": 12,
    "total_shutdowns": 10,
    "total_tasks_created": 45,
    "total_tasks_completed": 40,
    "total_tasks_failed": 5,
    "total_ask_calls": 78,
    "total_ask_errors": 2,
    "total_autopilot_runs": 5,
    "total_ralph_runs": 3
  }
}
```

`total_spawns` is a legacy compatibility alias for `total_team_runs`.

## Chat API

`omk chat` (alias `omk c`) opens a terminal-native REPL. Session state is persisted under the OMK state directory.

### Session files

| File | Purpose |
|------|---------|
| `~/.local/state/omk/sessions/<session-id>/meta.json` | Session metadata (id, start time, project root, theme) |
| `~/.local/state/omk/sessions/<session-id>/conversation.jsonl` | Append-only conversation log |
| `~/.local/state/omk/sessions/<session-id>/session-history.jsonl` | Input history for the current session |
| `~/.local/state/omk/sessions/<session-id>/engine-events.jsonl` | Engine pane event stream |

### `conversation.jsonl` schema

Each line is a JSON object:

```json
{"ts":"2026-05-21T10:00:00Z","role":"user","text":"hello"}
```

- `ts` — ISO 8601 UTC timestamp
- `role` — `user` or `assistant`
- `text` — message content

### CLI options

- `omk chat --session <id>` — resume a specific session
- `omk chat --new` — start a fresh session

## MCP Client API

`omk mcp` provides a client surface for configured MCP servers.

| Command | Output | Notes |
|---|---|---|
| `omk mcp list` | Text list of configured servers and their config path | Reads `~/.config/omk/mcp.json` or `.omk/mcp.json` |
| `omk mcp doctor` | Text diagnostics (healthy/unhealthy counts) | Checks reachability of configured transports |
| `omk mcp call <server> <tool> [args]` | Tool-specific output | `args` defaults to `{}` |

## Goal API

> **Current status:** Goal is CLI-only. There are no goal-specific MCP tools or REST endpoints yet. The web dashboard does not expose goal state over HTTP. Machine-readable access is available through the `omk goal` CLI with `--json` or `--format json|md`.

### Goal CLI — Machine-Readable Output

The following commands support structured output for scripting and integration:

| Command | `--json` | `--format text` | `--format json` | `--format md` | Notes |
| --- | --- | --- | --- | --- | --- |
| `omk goal show` | ✓ | ✓ | ✓ | ✓ | Full `GoalState` |
| `omk goal proof` | ✓ | ✓ | ✓ | ✓ | `GoalProof` artifact |
| `omk goal replay` | ✓ | ✓ | ✓ | ✓ | `GoalReplay` timeline |
| `omk goal budget` | ✓ | ✓ | ✓ | ✓ | `GoalBudgetReport` |
| `omk goal status` | — | — | — | — | Text only |
| `omk goal list` | — | — | — | — | Text only |

#### `omk goal show --json`

Serializes the full `GoalState` (secrets redacted at the Wire boundary):

```json
{
  "version": 1,
  "goal_id": "goal-20260519-abc123",
  "original_goal": "refactor authentication module",
  "normalized_goal": "refactor authentication module",
  "status": "running",
  "phase": "execution",
  "created_at": "2026-05-19T10:00:00Z",
  "updated_at": "2026-05-19T10:30:00Z",
  "completed_at": null,
  "until_ready": true,
  "budget_time": "8h",
  "budget_tokens": 500000,
  "budget_usd": 10.0,
  "max_agents": 5,
  "terminal_criteria": {
    "proof_required": true,
    "gates_required": true,
    "human_blockers_stop": true
  },
  "delivery_policy": "local",
  "merge_policy": "disabled",
  "slice_execution": false,
  "artifacts": [
    {
      "kind": "plan",
      "path": "/home/user/.local/state/omk/goal/goal-20260519-abc123/plan.md",
      "created_at": "2026-05-19T10:05:00Z"
    }
  ],
  "failure": null,
  "state_dir": "/home/user/.local/state/omk/goal/goal-20260519-abc123"
}
```

**Status values:** `running`, `ready`, `not_ready`, `blocked_on_human`, `blocked_on_external`, `needs_more_budget`, `failed_infra`, `paused`, `cancelled`.

**Phase values:** `intake`, `planning`, `decomposition`, `execution`, `verification_design`, `proof`.

#### `omk goal proof --json`

Serializes the current `GoalProof` artifact:

```json
{
  "version": 1,
  "goal_id": "goal-20260519-abc123",
  "status": "not_ready",
  "readiness": "not ready: verification gates and bounded agent execution passed, but review/security evidence is missing",
  "summary": "Goal 'refactor authentication module' has 3 gate result(s) and remains not ready until all required execution and review evidence exists.",
  "generated_at": "2026-05-19T10:30:00Z",
  "artifacts": [
    {
      "kind": "plan",
      "path": "/home/user/.local/state/omk/goal/goal-20260519-abc123/plan.md",
      "created_at": "2026-05-19T10:05:00Z"
    }
  ],
  "task_graph_summary": {
    "total_tasks": 5,
    "pending_tasks": 1,
    "blocked_tasks": 0,
    "done_tasks": 4
  },
  "changed_files": [
    "src/auth/mod.rs",
    "src/auth/oauth.rs"
  ],
  "commits": [
    "a1b2c3d"
  ],
  "gates": [
    {
      "name": "cargo test",
      "passed": true,
      "stdout": "...",
      "stderr": "",
      "duration_ms": 12500,
      "required": true,
      "command_line": "cargo test",
      "exit_code": 0,
      "timed_out": false
    }
  ],
  "post_mutation_gates_ran": false,
  "known_gaps": [
    "review evidence has not run for this goal yet",
    "security review evidence has not run for this goal yet"
  ],
  "human_decisions_required": []
}
```

#### `omk goal replay --json`

Serializes the deduplicated goal timeline:

```json
{
  "version": 1,
  "goal_id": "goal-20260519-abc123",
  "status": "running",
  "phase": "execution",
  "generated_at": "2026-05-19T10:30:00Z",
  "event_count": 12,
  "task_graph_summary": {
    "total_tasks": 5,
    "pending_tasks": 1,
    "blocked_tasks": 0,
    "done_tasks": 4
  },
  "timeline": [
    {
      "index": 0,
      "ts": "2026-05-19T10:00:05Z",
      "kind": "goal_created",
      "actor": null,
      "summary": "status=running, phase=intake"
    },
    {
      "index": 3,
      "ts": "2026-05-19T10:15:00Z",
      "kind": "task_started",
      "actor": "agent-1",
      "summary": "task_id=goal-agent-execute"
    }
  ],
  "recovery_status": null,
  "known_gaps": [],
  "duplicate_events": 0,
  "parse_failures": 0
}
```

#### `omk goal budget --json`

Serializes the `GoalBudgetReport` with checkpoints:

```json
{
  "version": 1,
  "goal_id": "goal-20260519-abc123",
  "generated_at": "2026-05-19T10:30:00Z",
  "budget_time": "8h",
  "total_budget_secs": 28800,
  "budget_tokens": 500000,
  "used_tokens": 125000,
  "remaining_budget_tokens": 375000,
  "budget_usd": 10.0,
  "estimated_cost_usd": 2.5,
  "remaining_budget_usd": 7.5,
  "latest": {
    "version": 1,
    "goal_id": "goal-20260519-abc123",
    "label": "budget_extended",
    "status": "running",
    "phase": "execution",
    "recorded_at": "2026-05-19T10:20:00Z",
    "budget_time": "8h",
    "total_budget_secs": 28800,
    "elapsed_since_created_secs": 1200,
    "remaining_budget_secs": 27600,
    "budget_tokens": 500000,
    "used_tokens": 125000,
    "remaining_budget_tokens": 375000,
    "budget_usd": 10.0,
    "estimated_cost_usd": 2.5,
    "remaining_budget_usd": 7.5
  },
  "checkpoints": [
    {
      "version": 1,
      "goal_id": "goal-20260519-abc123",
      "label": "budget_extended",
      "status": "running",
      "phase": "execution",
      "recorded_at": "2026-05-19T10:20:00Z",
      "budget_time": "8h",
      "total_budget_secs": 28800,
      "elapsed_since_created_secs": 1200,
      "remaining_budget_secs": 27600,
      "budget_tokens": 500000,
      "used_tokens": 125000,
      "remaining_budget_tokens": 375000,
      "budget_usd": 10.0,
      "estimated_cost_usd": 2.5,
      "remaining_budget_usd": 7.5
    }
  ],
  "spent_usd": 2.5,
  "spent_tokens": 125000,
  "spent_seconds": 1200
}
```
