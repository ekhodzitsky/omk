# OMK API Reference

## MCP Server (JSON-RPC 2.0 over stdio)

Start the MCP server:

```bash
omk mcp-server
```

### Tools

#### `omk_team_spawn`

Spawn a team of Kimi agents.

**Parameters:**

| Name | Type | Description |
|------|------|-------------|
| `count` | integer | Number of workers |
| `role` | string | Worker role (e.g. `coder`) |
| `task` | string | Task description |

**Response:**

```json
{
  "status": "spawned",
  "name": "coder-a1b2",
  "count": 3,
  "role": "coder",
  "task": "refactor authentication"
}
```

#### `omk_team_status`

Get team status.

**Parameters:**

| Name | Type | Description |
|------|------|-------------|
| `name` | string | Team name |

**Response:**

```json
{
  "name": "coder-a1b2",
  "task": "refactor authentication",
  "phase": "Running",
  "workers": 3
}
```

#### `omk_team_shutdown`

Shutdown a team.

**Parameters:**

| Name | Type | Description |
|------|------|-------------|
| `name` | string | Team name |

**Response:**

```json
{
  "status": "shutdown",
  "name": "coder-a1b2"
}
```

#### `omk_doctor`

Run environment diagnostics.

**Parameters:** None

**Response:**

```json
{
  "status": "ok",
  "checks": {
    "tmux": true,
    "kimi": true,
    "config_dir": "/home/user/.config/omk"
  }
}
```

## Web Dashboard REST API

Base URL: `http://localhost:8080`

### Endpoints

#### `GET /api/health`

Health check.

```json
{
  "status": "ok",
  "version": "0.2.5",
  "timestamp": "2026-05-08T12:00:00Z"
}
```

#### `GET /api/teams`

List active teams.

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

#### `GET /api/autopilots`

List active autopilot sessions.

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

#### `GET /api/ralphs`

List active Ralph sessions.

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

#### `GET /api/metrics`

Aggregated metrics.

```json
{
  "teams_spawned": 12,
  "teams_shutdown": 10,
  "tasks_completed": 45,
  "ask_calls": 78,
  "autopilot_runs": 5,
  "ralph_runs": 3
}
```
