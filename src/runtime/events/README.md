---
schema_version: 1
module: runtime::events
level: subsystem
purpose: Append-only JSONL event log with typed envelope, async writer actor, and resilient reader.
status: stable
surface:
  - name: RunId
    kind: struct
    visibility: pub
    contract: Unique run identifier. Display-formatted as `run-YYYYMMDD-HHMMSS-mmm`.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: EventId
    kind: struct
    visibility: pub
    contract: UUID-v4 identifier for a single event. Guaranteed unique within a run.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: WorkerId
    kind: struct
    visibility: pub
    contract: Opaque string identifier for a worker.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: TaskId
    kind: struct
    visibility: pub
    contract: Opaque string identifier for a task.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: GateId
    kind: struct
    visibility: pub
    contract: Opaque string identifier for a verification gate.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: EVENT_SCHEMA_VERSION
    kind: const
    visibility: pub
    contract: Current event envelope schema version. Bumped only when the Event struct shape changes.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: Event
    kind: struct
    visibility: pub
    contract: Common event envelope with id, run_id, ts, schema_version, kind, actor, and optional JSON payload.
    proof:
      kind: unit-test
      target: runtime::events::tests::event_roundtrip
      command: cargo test --lib runtime::events event_roundtrip
  - name: EventKind
    kind: enum
    visibility: pub
    contract: Discriminated union of all event kinds. Serialized as snake_case. New variants are backward-compatible for readers that tolerate unknown kinds.
    proof:
      kind: unit-test
      target: runtime::events::tests::event_serde_roundtrip_across_kinds_and_actor_shapes
      command: cargo test --lib runtime::events event_serde_roundtrip_across_kinds_and_actor_shapes
  - name: RunStartedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for RunStarted events. Includes optional Kimi metadata fields.
    proof:
      kind: unit-test
      target: runtime::events::tests::run_started_can_include_kimi_metadata
      command: cargo test --lib runtime::events run_started_can_include_kimi_metadata
  - name: WorkerStartedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for WorkerStarted events.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: WorkerHeartbeatPayload
    kind: struct
    visibility: pub
    contract: Typed payload for WorkerHeartbeat events.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: TaskClaimedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for TaskClaimed events. Includes lease_deadline.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: TaskCompletedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for TaskCompleted events.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: TaskGraphMutationPayload
    kind: struct
    visibility: pub
    contract: Typed payload for TaskGraphMutated events.
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: FileChangedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for FileChanged events. Operation is one of "created", "modified", "deleted".
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: CommandStartedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for CommandStarted events.
    proof:
      kind: unit-test
      target: runtime::events::tests::command_and_gate_events_can_include_evidence_payload
      command: cargo test --lib runtime::events command_and_gate_events_can_include_evidence_payload
  - name: CommandFinishedPayload
    kind: struct
    visibility: pub
    contract: Typed payload for CommandFinished events. Includes optional exit_code, timed_out flag, and output summaries.
    proof:
      kind: unit-test
      target: runtime::events::tests::command_and_gate_events_can_include_evidence_payload
      command: cargo test --lib runtime::events command_and_gate_events_can_include_evidence_payload
  - name: GateResultPayload
    kind: struct
    visibility: pub
    contract: Typed payload for GatePassed and GateFailed events. Carries full gate evidence.
    proof:
      kind: unit-test
      target: runtime::events::tests::command_and_gate_events_can_include_evidence_payload
      command: cargo test --lib runtime::events command_and_gate_events_can_include_evidence_payload
  - name: ProofWrittenPayload
    kind: struct
    visibility: pub
    contract: Typed payload for ProofWritten events. Status is one of "ready", "not_ready", "failed".
    proof:
      kind: unit-test
      target: runtime::events::tests
      command: cargo test --lib runtime::events
  - name: EventBuilder
    kind: struct
    visibility: pub
    contract: Convenience builder for common event patterns. Bound to a RunId at construction. Returns `anyhow::Result<Event>` for fallible payload serialization.
    proof:
      kind: unit-test
      target: runtime::events::tests::event_builder_helpers
      command: cargo test --lib runtime::events event_builder_helpers
  - name: EventReader
    kind: struct
    visibility: pub
    contract: Stateless reader for JSONL event logs. Tolerates missing files, blank lines, partial trailing lines, and malformed JSON. Never panics on corrupt input.
    proof:
      kind: unit-test
      target: runtime::events::tests::reader_tolerates_malformed_lines
      command: cargo test --lib runtime::events reader_tolerates_malformed_lines
  - name: EventLogSummary
    kind: struct
    visibility: pub
    contract: Summary statistics returned by EventReader::summary.
    proof:
      kind: unit-test
      target: runtime::events::tests::reader_summary
      command: cargo test --lib runtime::events reader_summary
  - name: payload_string
    kind: fn
    visibility: pub(crate)
    contract: Extract a string value from an event payload by key, with fallback to value["0"] for wrapped identifiers.
    proof:
      kind: unit-test
      target: runtime::events::tests::reader_edge_cases
      command: cargo test --lib runtime::events payload_string
  - name: JsonlWriter
    kind: struct
    visibility: pub
    contract: Low-level append-only JSONL writer backed by an mpsc actor. Guarantees line-atomic writes even with concurrent clones.
    proof:
      kind: unit-test
      target: runtime::events::writer::tests::concurrent_producers_do_not_interleave_lines
      command: cargo test --lib runtime::events concurrent_producers_do_not_interleave_lines
  - name: EventWriter
    kind: struct
    visibility: pub
    contract: High-level wrapper around JsonlWriter that serializes Event structs. Supports single and batch append.
    proof:
      kind: unit-test
      target: runtime::events::tests::writer_reader_roundtrip
      command: cargo test --lib runtime::events writer_reader_roundtrip
dependencies:
  internal: []
  external:
    - name: serde / serde_json
      scope: serialization
      reason: Event envelope and all payloads are JSON-serializable.
    - name: chrono
      scope: timestamps
      reason: Event timestamps and lease deadlines.
    - name: uuid
      scope: EventId generation
      reason: UUID-v4 for unique event identifiers.
    - name: tokio
      scope: async I/O and channels
      reason: EventWriter uses tokio::fs, mpsc, and oneshot for the writer actor.
    - name: anyhow
      scope: error handling
      reason: Fallible payload serialization and I/O errors.
    - name: tracing
      scope: diagnostics
      reason: Warn-level logging for malformed lines in EventReader.
consumers:
  - path: cli/team/run.rs
    uses: [Event, EventBuilder, EventKind, EventWriter, GateId, RunId]
  - path: cli/team/run_support.rs
    uses: [Event, EventBuilder, EventKind, EventWriter, RunId, WorkerId]
  - path: cli/team/manage.rs
    uses: [EventKind, EventWriter, RunId]
  - path: cli/team/proof.rs
    uses: [EventBuilder, EventWriter, RunId, Event, EventKind, EventReader]
  - path: runtime/scheduler/runner
    uses: [Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId]
  - path: runtime/wire_worker
    uses: [EventWriter, RunId, JsonlWriter, Event, EventBuilder, EventKind, TaskId, WorkerId]
  - path: runtime/proof
    uses: [EventReader, RunId, EventBuilder, GateId, WorkerId]
  - path: runtime/goal
    uses: [EventBuilder, EventWriter, RunId, Event, EventKind, GateId, TaskId, WorkerId]
  - path: vis/hud
    uses: [Event, EventKind, EventReader, RunId]
  - path: vis/event_stream.rs
    uses: [Event, EventReader, EventWriter]
  - path: tests/
    uses: [EventReader, EventWriter, EventKind]
invariants:
  - id: jsonl-line-atomicity
    rule: Concurrent EventWriter clones must never produce interleaved or partial JSON lines.
    proof:
      kind: unit-test
      target: runtime::events::writer::tests::concurrent_producers_do_not_interleave_lines
      command: cargo test --lib runtime::events concurrent_producers_do_not_interleave_lines
  - id: reader-resilience
    rule: EventReader::read_all must never panic or return Err for missing files, blank lines, partial lines, or malformed JSON.
    proof:
      kind: unit-test
      target: runtime::events::tests::reader_tolerates_malformed_lines
      command: cargo test --lib runtime::events reader_tolerates_malformed_lines
  - id: append-only
    rule: The event log is strictly append-only. No module in this subsystem provides delete, update, or truncate operations.
    proof:
      kind: static-check
      target: src/runtime/events/
      command: "! grep -rE 'truncate|remove_file|delete|clear' src/runtime/events/"
  - id: schema-version-stable
    rule: EVENT_SCHEMA_VERSION must not change without a coordinated migration across all consumers.
    proof:
      kind: static-check
      target: src/runtime/events/kind.rs
      command: "grep -q 'EVENT_SCHEMA_VERSION: u32 = 1' src/runtime/events/kind.rs"
  - id: unique-event-id
    rule: Every generated EventId must be a valid UUID-v4 and unique within practical bounds.
    proof:
      kind: unit-test
      target: runtime::events::tests::event_roundtrip
      command: cargo test --lib runtime::events event_roundtrip
  - id: unknown-kind-tolerance
    rule: A JSONL line with an unknown EventKind variant must be treated as a parse failure (skipped), not a panic or coercion.
    proof:
      kind: unit-test
      target: runtime::events::tests::reader_edge_cases::reader_skips_unknown_event_kind
      command: cargo test --lib runtime::events reader_skips_unknown_event_kind
  - id: no-super-imports
    rule: "No file in this module uses super::super:: imports."
    proof:
      kind: static-check
      target: src/runtime/events/
      command: "! grep -rE 'super::super::' src/runtime/events/"
  - id: storefront-small
    rule: The module entry file (events.rs) must remain a storefront under 100 lines.
    proof:
      kind: static-check
      target: src/runtime/events.rs
      command: "test $(wc -l < src/runtime/events.rs) -le 100"
verification:
  pre_change:
    - cargo test --lib runtime::events
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::events

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        consumers                            │
│   cli/team   runtime/goal   runtime/scheduler   vis/hud    │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                      runtime::events                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐ │
│  │   id     │  │  kind    │  │  builder │  │   reader   │ │
│  │(RunId,   │  │(Event,   │  │(Event-  │  │(EventReader│ │
│  │ EventId, │  │ EventKind│  │ Builder) │  │ EventLog-  │ │
│  │ WorkerId,│  │ payloads)│  │          │  │  Summary)  │ │
│  │ TaskId,  │  │          │  │          │  │            │ │
│  │ GateId)  │  │          │  │          │  │            │ │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘ │
│                              ┌──────────┐                 │
│                              │  writer  │                 │
│                              │(Jsonl-   │                 │
│                              │ Writer,  │                 │
│                              │Event-    │                 │
│                              │ Writer)  │                 │
│                              └──────────┘                 │
└─────────────────────────────────────────────────────────────┘
```

The events subsystem owns the **append-only JSONL event log** contract.
It is intentionally self-contained: it does not depend on any other OMK
runtime module, making it safe to import from anywhere in the crate
without circular dependency risk.

**Data flow:**
1. Producers (scheduler, goal lifecycle, wire workers) create `Event` values via `EventBuilder` or directly.
2. `EventWriter` serializes to JSON and sends bytes to a single `JsonlWriter` actor via an mpsc channel.
3. The actor appends lines to a file with `tokio::fs`, keeping the handle open for the actor's lifetime.
4. Consumers (HUD, proof generator, CLI inspect commands) read the log via `EventReader`, which tolerates corruption and missing files.

**Design choices:**
- **mpsc actor for writes:** Guarantees line-atomicity across concurrent producers without relying on filesystem O_APPEND semantics. Previously open-write-close per call could interleave on some filesystems.
- **Stateless reader:** No file handle caching; each read call opens, reads, and closes. This keeps the reader simple and avoids lock coordination with the writer process.
- **Typed payloads + loose envelope:** The `Event` envelope is strongly typed, but payloads are `serde_json::Value` at runtime. Typed payload structs (`RunStartedPayload`, etc.) provide ergonomic builder methods and deserialization helpers.
- **Resilience over strictness:** The reader skips malformed lines rather than failing the entire read. This lets partial writes or crashes leave recoverable logs.

## Files

| File | Owns |
| --- | --- |
| `events.rs` | Module entry — storefront re-exports only (14 lines). |
| `id.rs` | Identifier newtypes: `RunId`, `EventId`, `WorkerId`, `TaskId`, `GateId`. |
| `kind.rs` | `Event` envelope, `EventKind` enum, `EVENT_SCHEMA_VERSION`, and all typed payload structs. |
| `builder.rs` | `EventBuilder` — convenience constructors for common event patterns. |
| `reader.rs` | `EventReader`, `EventLogSummary`, and `payload_string` helper. |
| `writer.rs` | `JsonlWriter` (mpsc actor) and `EventWriter` (serialization wrapper). |
| `tests.rs` | Core roundtrip, concurrent, builder, and filter tests. |
| `tests/reader_edge_cases.rs` | Resilience tests: CRLF, whitespace, unknown kinds, range queries, payload_string branches. |

## Edit Rules

- `events.rs` is a storefront. Keep it under 100 lines.
- No `super::super::` imports. Use absolute `crate::runtime::events::` paths.
- New `EventKind` variants are additive-only; do not reorder existing variants.
- New typed payloads must derive `Serialize + Deserialize` and use `serde(default, skip_serializing_if = "Option::is_none")` for optional fields.
- If you change `EVENT_SCHEMA_VERSION`, update this contract and all consumer proof targets.
