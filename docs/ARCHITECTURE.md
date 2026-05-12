# OMK Architecture

OMK is a local Rust orchestration runtime for Kimi CLI. Kimi remains the model
execution engine; OMK owns scheduling, durable state, verification gates,
observability, and proof artifacts.

The north-star architecture adds `omk goal`: a durable controller that plans a
large outcome, builds a task graph, launches bounded agents, verifies results,
and writes a proof-backed terminal status. See `SPEC.md` for the product spec.
That controller is a local repository runtime, not a hosted coding-agent clone,
visual workflow builder, or IDE assistant. Competitive boundaries are tracked in
`docs/COMPETITIVE_POSITIONING.md`.

## Design Principles

1. **External orchestrator**: OMK never forks or patches Kimi CLI. It starts and
   coordinates `kimi --wire` processes from the outside.
2. **Wire-first worker control**: Team work runs through the Kimi Wire Protocol,
   not terminal-pane automation.
3. **Durable file state**: Runs, workers, claims, events, gates, proofs, and
   failures are written to disk so interrupted work can be inspected.
4. **Proof before done**: A run is not treated as ready until required gates and
   completion artifacts exist.

## System Context

```text
User
  |
  v
omk CLI (Rust)
  |
  +-- Kimi asset sync/install/doctor/rollback
  |
  +-- goal controller (early scaffold)
  |     |
  |     +-- PRD, technical plan, test spec
  |     +-- task graph, budgets, decisions
  |     +-- bounded scheduler/Wire execution waves
  |     +-- goal proof or failure
  |
  +-- team run scheduler
        |
        +-- run manifest, task claims, ownership map
        +-- workers/*/inbox.jsonl
        +-- workers/*/outbox.jsonl
        +-- workers/*/heartbeat.json
        +-- events.jsonl
        +-- proof.json or failure.json
        |
        +-- Wire worker tasks
              |
              +-- kimi --wire
```

## Runtime Modules

| Module | Purpose |
| --- | --- |
| `runtime/config.rs` | XDG path resolution with legacy `~/.omk/` fallback. |
| `runtime/state.rs` | JSON state machines for Team, Autopilot, and Ralph modes. |
| `runtime/migrate.rs` | State schema migration and future-version rejection. |
| `runtime/atomic.rs` | Atomic file writes via tempfile plus rename. |
| `runtime/retry.rs` | Exponential backoff helpers for resilient local operations. |
| `runtime/metrics.rs` | Telemetry counters persisted under the state directory. |
| `runtime/shell.rs` | Shell escaping and validation helpers. |
| `runtime/worker.rs` | Worker specs, inbox/outbox helpers, heartbeats. |
| `runtime/wire_worker.rs` | Worker loop that launches `kimi --wire` and records results. |
| `runtime/scheduler/` | Task decomposition, claims, leases, retries, and ownership checks. |
| `runtime/events.rs` | Append-only JSONL event envelope and readers. |
| `runtime/gates.rs` | Verification gate config, execution, and evidence capture. |
| `runtime/proof.rs` | Proof/failure report generation from events and gates. |
| `runtime/watchdog.rs` | State-file health checks for workers and stale heartbeats. |
| `runtime/goal.rs` | Goal controller scaffold, task graph, local gates, policy-validated bounded agent waves, agent-proposed follow-up tasks, and proof state. |

## Data Flow

1. User runs `omk team run 3:executor "task"`.
2. OMK creates a team state directory under the active XDG/legacy state root.
3. The scheduler decomposes the task and writes claimable tasks.
4. Workers claim tasks, receive inbox records, run `kimi --wire`, and append
   outbox records plus heartbeat evidence.
5. OMK appends run, worker, task, Wire, gate, and failure events to `events.jsonl`.
6. Required verification gates run and capture stdout/stderr artifacts.
7. OMK writes `proof.json` for ready runs or `failure.json` for failed,
   interrupted, or not-ready runs.
8. Operators inspect the result with `omk run show`, `omk proof show`,
   `omk hud`, or `omk team health`.

Current `omk goal` scaffold data flow:

1. User runs `omk goal run "large outcome" --until-ready`.
2. OMK writes `goals/<goal-id>/goal.json` under the OMK state directory.
3. OMK writes `prd.md`, `technical-plan.md`, `test-spec.md`, and
   `task-graph.json`.
4. OMK writes an honest `proof.json` with `not_ready` until required
   execution, review, and hardening evidence exists.
5. OMK marks controller-owned planning tasks as done with artifact evidence and
   writes goal-level task events to `events.jsonl`.
6. OMK captures best-effort git branch, HEAD commit, dirty-state, and changed
   files for the proof bundle.
7. `omk goal verify` runs local gates, writes full gate output under
   `artifacts/gates/`, appends gate events, and refreshes `proof.json`.
8. `omk goal execute` marks `goal-local-verify` done when required gates pass,
   turns `goal-agent-execute` into a controller-proposed multi-task Wire worker
   wave, validates proposals against policy and per-task budgets, emits
   `task_proposed`, `task_accepted`, and `task_rejected`, and records
   `task-policy.json`, outbox, Wire event, mutation diff, and changed-file
   evidence under `artifacts/agent-runs/`. Workers may return structured
   `OMK_TASK_PROPOSAL: {...}` follow-up work; the controller records
   `agent-task-proposals.json` and appends accepted safe proposals as pending
   task graph nodes. If the worker changes project files, `execute` reruns
   gates under `artifacts/gates/post-mutation/` before writing the final proof.
9. `omk goal review` writes controller review/security artifacts under
   `artifacts/reviews/` and closes `goal-review` / `goal-security-review`
   when evidence is sufficient.
10. Operators inspect with `omk goal list/status/show/proof`.
11. `omk goal cancel` writes `failure.json`.

Planned later flow adds multi-task execution waves, task graph mutation,
specialist review loops, integration acceptance, and ready proof generation.

## CLI Surfaces

| Surface | Role |
| --- | --- |
| `omk kimi ...` | Manage Kimi-native agents, hooks, skills, manifests, backups, and drift checks. |
| `omk team run` | Run the scheduler-backed Wire team workflow. |
| `omk team status/health/shutdown/cleanup` | Inspect and manage durable team state. |
| `omk run show/list` | Inspect event timelines and run metadata. |
| `omk proof show` | Inspect cached or regenerated readiness evidence. |
| `omk hud` | Render text, JSON, TUI, or web status views. |
| `omk autopilot`, `omk ralph`, `omk ultrawork` | Power-user execution modes built on the same local runtime expectations. |
| `omk goal ...` | Current scaffold for durable goal state, planning artifacts, task graph with controller-owned, local verification, policy-validated multi-task Wire agent mutation, accepted agent-proposed follow-up tasks, post-mutation gate reruns, review, and security evidence, git evidence, local gate evidence, and not-ready proof; planned controller for long-running proof-backed engineering goals. |

## MCP Integration

The MCP server exposes a small CLI-backed surface:

- `omk_team_run`
- `omk_team_status`
- `omk_team_shutdown`
- `omk_doctor`

The MCP tools delegate to the same CLI commands so behavior stays aligned with
local terminal usage.

## Skill System

Skills are markdown files with YAML frontmatter, discovered from:

1. `.omk/skills/` (project scope, legacy)
2. `~/.local/share/omk/skills/` (user scope, XDG)
3. `<omk binary dir>/skills/` (bundled)

Kimi-native assets under `.kimi/` are the preferred current path for agents,
hooks, and skills that should be available directly to Kimi CLI.
