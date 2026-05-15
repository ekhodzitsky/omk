---
schema_version: 1
module: runtime::wire_worker
level: subsystem
purpose: Adapts a WorkerSpec to the Kimi Wire Protocol by polling an inbox, spawning a kimi CLI child, processing wire messages, and writing results to an outbox with full event emission.
status: pilot
surface:
  - name: WireWorkerAdapter
    kind: struct
    visibility: pub
    contract: |
      Owns a WorkerSpec and runs a background loop that consumes JSONL tasks from an inbox,
      drives each task through a kimi --wire child process, and writes WorkerResult lines to an outbox.
      Emits runtime events for task start, completion, failure, stall, wire initialization,
      wire turn timeout, and wire request/response pairs.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter
  - name: WireWorkerAdapter::new
    kind: fn
    visibility: pub
    contract: Constructs an adapter with a default CancellationToken and an environment-resolved active turn timeout.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter
  - name: WireWorkerAdapter::new_with_cancel
    kind: fn
    visibility: pub
    contract: Constructs an adapter with an external CancellationToken so the caller can trigger graceful shutdown.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter_cancellation_stops_idle_worker
  - name: WireWorkerAdapter::spawn
    kind: fn
    visibility: pub
    contract: Spawns the adapter as a background Tokio task that runs until cancelled or encounters a fatal error.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter_cancellation_stops_idle_worker
  - name: POLL_INTERVAL_SECS
    kind: const
    visibility: pub
    contract: Default 5-second poll interval for inbox checks. Override via OMK_WIRE_WORKER_POLL_INTERVAL_MS or OMK_WIRE_WORKER_POLL_INTERVAL_SECS.
    proof:
      kind: static-check
      target: src/runtime/wire_worker.rs
      command: cargo check
  - name: poll_interval
    kind: fn
    visibility: pub(crate)
    contract: Resolves the poll interval from environment variables, falling back to POLL_INTERVAL_SECS.
    proof:
      kind: static-check
      target: src/runtime/wire_worker.rs
      command: cargo check
dependencies:
  internal:
    - module: runtime::events
      scope: Event, EventBuilder, EventKind, EventWriter, JsonlWriter, RunId, TaskId, WorkerId
      reason: All task lifecycle transitions and wire traffic must be observable through the event stream.
    - module: runtime::worker
      scope: WorkerSpec, WorkerTask, WorkerResult, ResultStatus
      reason: The adapter consumes and produces the worker IPC types defined by the runtime.
    - module: wire::client
      scope: ProcessWireClient, WireClient, WireMessage
      reason: Drives the kimi child process via the Wire client abstraction.
    - module: wire::protocol
      scope: redact_wire_secrets, Request, RequestParams, KIMI_WIRE_PROTOCOL_VERSION, InitializeParams, ClientInfo, Event
      reason: Must speak the Kimi Wire Protocol and redact secrets before logging or event emission.
  external:
    - name: tokio / tokio_util
      scope: async runtime, CancellationToken, async file I/O
      reason: Background task lifecycle and async filesystem operations.
    - name: tracing
      scope: structured logging
      reason: Observability of worker and task lifecycle.
    - name: anyhow
      scope: error propagation
      reason: Ergonomic Result types across async boundaries.
    - name: serde_json
      scope: serialization
      reason: Inbox/outbox lines and wire message payloads are JSON.
    - name: chrono
      scope: heartbeat timestamps
      reason: RFC 3339 timestamps in heartbeat files.
    - name: which
      scope: binary discovery
      reason: Locates the kimi executable on PATH when MOCK_KIMI is not set.
consumers:
  - path: src/cli/team/run_support.rs
    uses: ["WireWorkerAdapter::new_with_cancel"]
  - path: src/runtime/goal/dispatch/tasks/wave.rs
    uses: ["WireWorkerAdapter::new_with_cancel"]
  - path: tests/mock_kimi_test.rs
    uses: ["WireWorkerAdapter::new_with_cancel"]
  - path: tests/fixtures/team_demo_fixture.rs
    uses: ["WireWorkerAdapter"]
invariants:
  - id: cancellation-graceful-shutdown
    rule: When the CancellationToken fires, the adapter writes a stopped heartbeat and exits without panic.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter_cancellation_stops_idle_worker
  - id: stalled-turn-timeout
    rule: If a wire turn exceeds the active turn timeout, the adapter records a failed WorkerResult and emits a WorkerStalled event.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter_times_out_stalled_turn_and_writes_failed_result
  - id: task-budget-timeout
    rule: If a task exceeds its per-task budget, the adapter kills the kimi child and writes a timeout WorkerResult to the outbox.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter_enforces_task_budget_timeout
  - id: mid-task-crash-recovery
    rule: If the kimi child crashes mid-task, the adapter records a failure reason and exits the task loop cleanly.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test test_wire_worker_adapter_handles_mid_task_crash_after_turn_begin
  - id: no-super-super-imports
    rule: "Sub-files use absolute crate:: paths; no super::super:: imports exist."
    proof:
      kind: static-check
      target: src/runtime/wire_worker/
      command: "! grep -r 'super::super::' src/runtime/wire_worker/"
  - id: secrets-redacted
    rule: Wire request payloads and responses are passed through redact_wire_secrets before event emission.
    proof:
      kind: static-check
      target: src/runtime/wire_worker/task.rs
      command: grep -n "redact_wire_secrets" src/runtime/wire_worker/task.rs
verification:
  pre_change:
    - cargo test --lib runtime::wire_worker
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::wire_worker

## Architecture

`WireWorkerAdapter` is a bridge between OMK's JSONL worker IPC and the Kimi Wire Protocol.

The adapter runs as a single background Tokio task with three concurrent concerns:

1. **Inbox polling** — reads `WorkerTask` lines from a JSONL file, tracking the last consumed
   byte offset so restarts are safe and new tasks are append-only.
2. **Wire session per task** — spawns `kimi --wire`, performs `initialize` handshake, sends the
   task prompt, then reads wire messages until `turn_end`, `step_interrupted`, error, or timeout.
3. **Outbox & events** — writes `WorkerResult` lines to the outbox and appends structured
   `Event` records through a shared `EventWriter`.

Cancellation is cooperative. The outer loop observes a `CancellationToken` and writes a
heartbeat file (`ready` → `alive` → `stopped`). Each task also gets a per-task timeout
token so budget enforcement is race-free against external shutdown.

The entry file (`src/runtime/wire_worker.rs`) is a storefront: it declares the two
implementation submodules (`loop_impl`, `task`), exposes the public surface, and keeps
business logic in the named implementation files.

## Files

| File | Owns |
| --- | --- |
| `wire_worker.rs` | Public surface: `WireWorkerAdapter`, `POLL_INTERVAL_SECS`, `poll_interval`. Environment resolution for timeouts and poll intervals. |
| `wire_worker/loop_impl.rs` | The `run_loop` method: inbox seek/read, heartbeat maintenance, task dispatch, and shutdown sequencing. |
| `wire_worker/task.rs` | The `process_task` method: wire client lifecycle, message routing, outbox result construction, and timeout/cancellation handling. Also `record_task_timeout`, `record_wire_request`, and `write_worker_result`. |
