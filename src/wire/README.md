---
schema_version: 1
module: wire
level: root
purpose: Kimi Wire Protocol client and JSON-RPC types
status: stable
surface:
  - name: WireClient
    kind: trait
    visibility: pub
    contract: Contract for wire protocol clients. All high-level methods (prompt, replay, steer, cancel, set_plan_mode) have default impls built on low-level primitives.
    proof:
      kind: unit-test
      target: wire::client
      command: cargo test --lib wire::client
  - name: ProcessWireClient
    kind: struct
    visibility: pub
    contract: Child-process implementation of WireClient. Spawns `kimi` binary and communicates over stdin/stdout.
    proof:
      kind: unit-test
      target: wire::client
      command: cargo test --lib wire::client
  - name: InMemoryWireClient
    kind: struct
    visibility: pub(crate)
    contract: In-memory mock for deterministic unit tests. No child process, no filesystem I/O.
    proof:
      kind: unit-test
      target: wire::client
      command: cargo test --lib wire::client
  - name: process_messages
    kind: fn
    visibility: pub(crate)
    contract: Generic message dispatch loop over impl WireClient. Not bound to any concrete client type.
    proof:
      kind: unit-test
      target: wire::client::dispatch
      command: cargo test --lib wire::client::dispatch
  - name: WireMessage
    kind: enum
    visibility: pub
    contract: Union of all incoming wire messages (request, response, event, error).
    proof:
      kind: unit-test
      target: wire::protocol
      command: cargo test --lib wire::protocol
  - name: WireResponse
    kind: struct
    visibility: pub
    contract: Response envelope sent back to the agent over the wire.
    proof:
      kind: unit-test
      target: wire::protocol
      command: cargo test --lib wire::protocol
dependencies:
  internal: []
  external: []
consumers:
  - path: runtime/wire_worker/task.rs
    uses: [ProcessWireClient, process_messages]
  - path: runtime/scheduler/decompose.rs
    uses: [ProcessWireClient, process_messages]
invariants:
  - id: generic-dispatch
    rule: process_messages is generic over WireClient, not bound to concrete type.
    proof:
      kind: unit-test
      target: wire::client::dispatch
      command: cargo test --lib wire::client::dispatch
  - id: default-impls
    rule: All protocol methods have default impl in trait using low-level primitives.
    proof:
      kind: unit-test
      target: wire::client
      command: cargo test --lib wire::client
  - id: mock-tests
    rule: Unit tests use InMemoryWireClient; only spawn integration tests use ProcessWireClient.
    proof:
      kind: unit-test
      target: wire::client
      command: cargo test --lib wire::client
verification:
  pre_change:
    - cargo test --lib wire::client
  full:
    - cargo test --test wire_protocol_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# wire

## Architecture

```
┌──────────────────────┐
│   process_messages   │  ← generic over WireClient
│   <C: WireClient>    │
└──────────┬───────────┘
           │ trait WireClient
      ┌────┴──────┐
      ▼             ▼
ProcessWire   InMemoryWire
Client        Client
(child proc)  (test mock)
```

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Wire module exports. |
| `protocol.rs` | JSON-RPC request/response/event types, protocol version helpers, parsing. |
| `client/client_trait.rs` | `WireClient` trait + `InMemoryWireClient` (test mock). |
| `client.rs` | `ProcessWireClient` — child-process implementation. |
| `client/dispatch.rs` | Generic `process_messages` loop over `impl WireClient`. |
| `client/io.rs` | `ProcessWireClient` I/O helper (`read_message_from_stdout`). |
| `client/spawn.rs` | `ProcessWireClient::spawn` constructor. |
| `client/process_impl.rs` | `impl WireClient for ProcessWireClient` (all trait methods). |
| `client/tests.rs` | Unit tests. Shell scripts only for spawn smoke; everything else uses `InMemoryWireClient`. |

## Edit Rules

- Check the official Kimi Wire docs before changing protocol fields, method names, or fallback behavior.
- Record Kimi CLI version and negotiated Wire protocol version when the runtime observes them.
- Prefer strongly typed protocol structs over ad hoc JSON strings.
- Keep prompt-scraping as a fallback path, not the primary contract.
- Preserve the legacy/no-handshake fallback for older upstream behavior.
- Add fixtures when changing parse behavior.
- Any new protocol method goes into the `WireClient` trait with a default impl.

## Tests

```bash
cargo test --lib wire::client
cargo test --test wire_protocol_test
bash -n scripts/kimi-wire-smoke.sh
```

If a local authenticated `kimi` binary is available, also run:

```bash
scripts/kimi-wire-smoke.sh
```

The smoke script is intentionally outside `cargo test` because it depends on the user's Kimi installation and auth state.
