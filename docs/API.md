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
  "version": "0.3.1",
  "timestamp": "2026-05-11T12:00:00Z"
}
```

### `GET /api/teams`

```json
[
  {
    "name": "coder-a1b2",
    "task": "refactor authentication",
    "phase": "Running",
    "worker_count": 3
  }
]
```

### `GET /api/autopilots`

```json
[
  {
    "name": "ap-xxx",
    "task": "build REST API",
    "phase": "Execution",
    "progress": 42
  }
]
```

### `GET /api/ralphs`

```json
[
  {
    "task": "implement auth",
    "iteration": 3,
    "max_iterations": 10,
    "verified_stories": 2,
    "total_stories": 5
  }
]
```

### `GET /api/metrics`

Aggregated runtime counters.

```json
{
  "total_team_runs": 12,
  "total_spawns": 12,
  "total_shutdowns": 10,
  "tasks_completed": 45,
  "ask_calls": 78,
  "autopilot_runs": 5,
  "ralph_runs": 3
}
```

`total_spawns` is a legacy compatibility alias for `total_team_runs`.
