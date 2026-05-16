---
schema_version: 1
module: runtime::scheduler
level: subsystem
purpose: Task decomposition, ownership, claims, and team runner scaffolding.
status: pilot
surface:
  - name: TeamRunner
    kind: struct
    visibility: pub
    contract: Orchestrates a team of workers through task claim, execution, and result collection.
    proof:
      kind: integration-test
      target: team_lifecycle_test
      command: cargo test --test team_lifecycle_test
  - name: Subtask
    kind: struct
    visibility: pub
    contract: Decomposed work unit with description and optional file ownership.
    proof:
      kind: integration-test
      target: team_lifecycle_test
      command: cargo test --test team_lifecycle_test
  - name: RunManifest
    kind: struct
    visibility: pub
    contract: Manifest of a team run with task list and metadata.
    proof:
      kind: integration-test
      target: team_lifecycle_test
      command: cargo test --test team_lifecycle_test
  - name: Task
    kind: struct
    visibility: pub
    contract: Scheduler task with state machine (pending, claimed, completed, failed).
    proof:
      kind: integration-test
      target: team_lifecycle_test
      command: cargo test --test team_lifecycle_test
  - name: OwnershipMap
    kind: struct
    visibility: pub
    contract: File ownership tracking to detect read-write hazards across concurrent tasks.
    proof:
      kind: integration-test
      target: team_lifecycle_test
      command: cargo test --test team_lifecycle_test
dependencies:
  internal:
    - module: runtime::events
      scope: event emission
      reason: TeamRunner emits TaskClaimed, TaskCompleted, and WorkerStarted events.
    - module: runtime::worker
      scope: worker IPC
      reason: Consumes WorkerSpec and produces WorkerResult via inbox/outbox.
    - module: runtime::wire_worker
      scope: wire adapter
      reason: Dispatches agent tasks through WireWorkerAdapter.
  external:
    - name: tokio
      scope: async runtime
      reason: Async task scheduling and file I/O.
    - name: serde
      scope: serialization
      reason: Task and manifest JSON serialization.
    - name: anyhow
      scope: error handling
      reason: Result propagation across the scheduler.
consumers:
  - path: src/cli/team/run.rs
    uses: [TeamRunner, RunManifest]
  - path: src/runtime/goal/dispatch
    uses: [Task, Subtask]
invariants:
  - id: ownership-conflict-detection
    rule: OwnershipMap flags ReadWriteHazard when two tasks claim overlapping write sets.
    proof:
      kind: static-check
      target: src/runtime/scheduler/ownership.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: claim-lease-expiry
    rule: Task claims include a lease deadline; expired claims are eligible for re-claim.
    proof:
      kind: static-check
      target: src/runtime/scheduler/claim
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib runtime::scheduler
  full:
    - cargo test --test team_lifecycle_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::scheduler

## Architecture

The `scheduler` module decomposes work, tracks ownership, and orchestrates worker execution.

- `decompose.rs` owns task decomposition into `Subtask` units.
- `manifest.rs` owns `RunManifest` and run event types.
- `ownership.rs` owns `OwnershipMap` and hazard detection.
- `task.rs` owns the `Task` state machine and retry policy.
- `worker_state.rs` owns `WorkerState` and `WorkerStateMap`.
- `claim/` owns task claim and lease logic.
- `runner/` owns the `TeamRunner` execution loop.

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Storefront: declares all submodules. |
| `decompose.rs` | Task decomposition into subtasks. |
| `manifest.rs` | Run manifest and event type definitions. |
| `ownership.rs` | File ownership and read-write hazard detection. |
| `task.rs` | Task state machine and retry policy. |
| `worker_state.rs` | Worker state tracking. |
| `claim/` | Task claim and lease management. |
| `runner/` | Team runner orchestration loop. |
