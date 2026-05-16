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
| `runtime/goal/` | Goal controller: durable state, validated task graph with retry/lease metadata, bounded Wire-backed execution waves with per-task budgets and optional per-slice worktree isolation, pause/resume/cancel with worker interruption, budget enforcement and recovery, per-slice PR delivery and review/fix loop, integrator branch/PR creation, controller narrative emission, post-mutation gate reruns, deterministic event replay, and proof state. |

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

```text
goal run/plan
  |
  v
intake + oracle classifier
  |
  +-- blocked_on_human -> failure.json + proof.json
  |
  v
planner artifacts -> task graph -> decisions
  |
  v
verify gates -> execute scheduler/Wire wave (worktrees if --slice-execution)
  |
  v
per-slice commit / push / PR / review (non-Local policy)
  |
  v
integrator branch + PR when all slices Delivered
  |
  v
review wall -> integrator accept/reject -> proof.json -> open-pr --dry-run
```

1. User runs `omk goal run "large outcome" --until-ready`.
2. OMK writes `goals/<goal-id>/goal.json` under the OMK state directory and
   reloads goal state with safe defaults plus actual-directory `state_dir`
   rehoming for restored or older records.
3. OMK writes `prd.md`, `technical-plan.md`, `test-spec.md`,
   `task-graph.json`, and `decisions.jsonl`.
4. Vague goals without testable success criteria stop as `blocked_on_human`,
   write `failure.json`, and record the required human decision in `proof.json`.
5. OMK writes an honest `proof.json` with `not_ready` until required
   execution, review, and hardening evidence exists.
6. OMK marks controller-owned planning tasks as done with artifact evidence,
   writes goal-level task events to `events.jsonl`, and records controller
   rationale in `decisions.jsonl`.
7. OMK captures best-effort git branch, HEAD commit, dirty-state, and changed
   files for the proof bundle.
8. `omk goal verify` runs local gates, writes full gate output under
   `artifacts/gates/`, appends gate events, and refreshes `proof.json`.
9. `omk goal execute` marks `goal-local-verify` done when required gates pass
   and runs the bounded `goal-agent-execute` Wire wave:
   - controller validates each task against policy, write scopes, and per-task
     budgets, emitting `task_proposed`/`task_accepted`/`task_rejected` events
     and recording `task-policy.json`, outbox, Wire, mutation diff, and
     changed-file evidence under `artifacts/agent-runs/`;
   - Wire workers may return structured `OMK_TASK_PROPOSAL: {...}` follow-up
     work, which the controller appends to the task graph only when policy and
     dependency-ordered read/write conflict checks pass, emitting
     `task_graph_mutated` events for accepted additions;
   - task graphs are validated on load for duplicate ids, missing or cyclic
     dependencies, and self-dependencies; nodes carry `retry_count`,
     `max_retries`, and `lease_expires_at`;
   - follow-up dispatches honor the goal `max_agents` cap, recover expired
     leases with `retry_scheduled` evidence, and quarantine stale workers with
     `worker_dead` evidence plus `stale-worker-cleanup.json` markers;
   - pause/cancel during an active wave cancels workers, halts dispatch, and
     preserves the interrupted goal/proof status;
   - project-file mutations trigger gate reruns under
     `artifacts/gates/post-mutation/` before the final proof.
10. `omk goal review` writes controller review/security artifacts under
   `artifacts/reviews/` and closes `goal-review` / `goal-security-review`
   when evidence is sufficient.
11. `omk goal accept` / `reject` records explicit local integration evidence;
   only accepted goals with gates, execution, review, oracle, mutation, and
   integration evidence can become `ready`.
12. `--slice-execution` runs each agent task in an isolated git worktree on a
    deterministic branch (`goal/{goal_id}/slice/{task_id}`), serializes
    overlapping write scopes, and cleans up worktrees on successful delivery.
    Delivery metadata (slice id, worktree path, branch, status, PR URL) is
    recorded in the task graph per task.
13. Per-slice PR delivery: when slice execution is combined with a non-Local
    `--policy`, each slice is auto-committed, pushed, and opened as a dedicated
    PR; a lightweight per-slice review runs gates and a security scan. Failed
    slices are reset to `Pending` with `[review-feedback]` injected into the
    next agent prompt for automatic retry.
14. Integrator PR: after all slices reach `Delivered`, the controller creates
    an `integrator/{goal_id}` branch from current master, merges all slice
    branches, pushes, and opens an integrator PR that follows the chosen
    `merge_policy` (`gated`, `manual`, or `disabled`).
15. Controller narrative: `run_goal_until_ready` emits `TaskOutput` events after
    each controller step; the CLI renders a numbered `Narrative:` section with
    emoji icons for plan, verify, execute, review, deliver, and blocked steps.
16. `omk goal open-pr --dry-run` renders a PR title/body from local proof
   evidence without pushing or creating a GitHub PR.
17. Operators inspect with `omk goal list/status/show/proof`, or attach a
   no-dependency watcher to the files documented in
   [GOAL_NOTIFICATIONS.md](GOAL_NOTIFICATIONS.md).
18. `omk goal cancel` writes `failure.json`.

Planned later flow keeps GitHub mutation explicit: draft PR creation may be
added, but dry-run rendering remains the default safe path.

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
| `omk goal ...` | Durable goal controller: planning artifacts, validated task graph with retry/lease metadata, bounded Wire-backed execution waves with optional per-slice worktree isolation, verification gates with post-mutation reruns, per-slice PR delivery and review/fix loop, integrator PR merging, controller narrative emission, budget enforcement, and honest proof artifacts. Local, repo-native, proof-backed runtime—not a hosted coding-agent clone. |

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
