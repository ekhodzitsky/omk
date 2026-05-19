# Runtime

`src/runtime/` is OMK's control plane. It owns durable state, process control,
scheduling, retries, verification evidence, and recovery behavior.

## Status

`pilot` — core team scheduler and event pipeline are stable. The `goal`
subsystem is an active MVP scaffold with end-to-end delivery in progress.

## Public API

### Types and Traits

| Name | Module | Purpose |
|------|--------|---------|
| `Event` | `events` | Append-only event envelope with kind, run id, and timestamp. |
| `EventBuilder` | `events` | Fluent builder for events before writing. |
| `EventKind` | `events` | Discriminant for all runtime event types. |
| `EventReader` | `events` | Streaming reader for `events.jsonl`. |
| `EventWriter` | `events` | Atomic appender for `events.jsonl`. |
| `EventSink` | `events` | Trait for event consumers (files, channels, in-memory mocks). |
| `RunId` | `events` | Newtype for a run/session identifier. |
| `WorkerId` | `events` | Newtype for a worker identifier. |
| `TaskId` | `events` | Newtype for a scheduler task identifier. |
| `WorkerSpec` | `worker` | Specification for a single team worker (inbox, outbox, role). |
| `WorkerResult` | `worker` | Structured result from a worker execution. |
| `Proof` | `proof` | Readiness report with gates, changed files, and verdict. |
| `ProofStatus` | `proof` | Enum: `ready`, `not_ready`, `blocked`, etc. |
| `ProofGenerator` | `proof` | Synthesizes a `Proof` from events and gate results. |
| `GateDef` | `gates` | Definition of a verification gate (command, timeout, required). |
| `GateResult` | `gates` | Outcome of a single gate execution. |
| `VerificationConfig` | `gates` | Aggregate gate configuration for a run. |
| `TeamState` | `state` | In-memory aggregate of team run status. |
| `TaskStatus` | `state` | Enum for scheduler task lifecycle. |
| `Watchdog` | `watchdog` | Detects stuck or dead workers via heartbeat timeouts. |
| `WorkerHealth` | `watchdog` | Per-worker health record. |
| `HealthStatus` | `watchdog` | Enum: `healthy`, `stalled`, `dead`. |
| `OmkConfig` | `config` | XDG-compliant configuration struct. |
| `ApprovalPolicy` | `wire_worker` | Policy for Wire tool-use approvals (auto, manual, never). |
| `WireWorkerAdapter` | `wire_worker` | Bridges a Kimi Wire worker into the scheduler. |

### Functions

| Name | Module | Purpose |
|------|--------|---------|
| `atomic_write` | `atomic` | Write a file atomically via temp+rename. |
| `omk_state_dir` | `config` | Returns `XDG_STATE_HOME/omk/`. |
| `sanitize_name` | `sanitize` | Sanitizes an identifier for filesystem use. |
| `retry` | `retry` | Retry a fallible async operation with exponential backoff. |
| `run_gates_with_evidence` | `gates` | Execute all configured gates and emit evidence events. |

## Dependencies

### Internal

| Module | Scope | Reason |
|--------|-------|--------|
| `wire` | `wire_worker` | Wire protocol types and client for spawning Kimi workers. |
| `cost` | `events` | Cost snapshot attachment to events. |

### External

| Crate | Scope | Reason |
|-------|-------|--------|
| `anyhow` | error handling | Result/context propagation across runtime boundaries. |
| `serde` / `serde_json` | serialization | State files, events, and proof are JSON. |
| `tokio` | async runtime | fs, process spawning, timeouts, channels. |
| `tracing` | observability | Structured logging for runs, workers, and gates. |
| `chrono` | timestamps | Event and proof timestamps. |

## Consumers

| Path | Uses |
|------|------|
| `src/cli/` | All runtime modules to implement CLI commands. |
| `src/mcp/` | `wire_worker`, `scheduler` for MCP tool execution. |
| `src/vis/` | `events`, `proof` for rendering progress and reports. |

## Files

### Root-level modules

| File | Owns |
| --- | --- |
| `atomic.rs` | Atomic file writes (temp+rename). |
| `config.rs` | XDG and legacy path resolution. |
| `metrics.rs` | Runtime metrics collection. |
| `migrate.rs` | State schema migration. |
| `retry.rs` | Retry/backoff helpers. |
| `sanitize.rs` | Identifier sanitization for filesystem paths. |
| `shell.rs` | Shell escaping and validation. |
| `state.rs` | Team/autopilot/ralph state files. |
| `ultrawork.rs` | Parallel burst execution runtime. |
| `watchdog.rs` | Dead/stuck worker detection. |
| `worker.rs` | JSONL inbox/outbox worker IPC and `WorkerSpec`. |

### Directory modules

| Directory | Owns |
|-----------|------|
| `ask/` | Provider ask execution helpers. |
| `autopilot/` | Autopilot state machine and execution loop. |
| `events/` | Event envelope, timeline, reader, writer, sink trait. |
| `gates/` | Verification gate definitions, execution, and evidence. |
| `goal/` | Goal controller: durable state, planning, execution waves, worktrees, PR delivery, review wall, budget, replay. |
| `proof/` | Proof/readiness report data and synthesis. |
| `ralph/` | Ralph persistence loop. |
| `scheduler/` | Task claims, leases, ownership, manifest, and runner scaffold. |
| `wire_worker/` | Kimi Wire worker adapter used by `team run`. |

## Edit Rules

- Runtime changes should be observable through state, events, logs, or proof output.
- Prefer append-only event records for behavior that must be audited later.
- Use `atomic.rs` for state/proof/event artifacts that readers may inspect while a run is active.
- Keep `team run` Wire-first; do not add terminal-session orchestration back into the runtime.
- Do not silently change state file shape without migration or a compatibility note.

## Tests

Useful starting points:

```bash
cargo test --test team_lifecycle_test
cargo test --test gates_test
cargo test --test proof_cmd_test
cargo test --test proof_golden_test
cargo test --test ultrawork_test
```

When touching state migration or filesystem layout, include a focused test with temporary HOME/XDG directories.
