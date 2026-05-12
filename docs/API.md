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
  "team": "coder-a1b2",
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
  "team": "coder-a1b2",
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
  "version": "0.3.4",
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
