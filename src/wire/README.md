---
schema_version: 1
module: wire
level: root
purpose: Re-export and compatibility layer for the `kimi-wire` crate
status: stable
surface:
  - name: WireClient
    kind: trait
    visibility: pub
    contract: Re-exported from `kimi-wire::client::WireClient`. Protocol methods (prompt, replay, steer, cancel, set_plan_mode) have default impls.
    proof:
      kind: unit-test
      target: kimi-wire::client
      command: cargo test --lib -p kimi-wire
  - name: ProcessWireClient
    kind: type alias
    visibility: pub
    contract: `TransportWireClient<ChildProcessTransport>`. Production wire client that spawns `kimi --wire`.
    proof:
      kind: integration-test
      target: tests/mock_kimi_test.rs
      command: cargo test --test mock_kimi_test
  - name: InMemoryWireClient
    kind: struct
    visibility: pub
    contract: Re-exported from `kimi-wire::client::InMemoryWireClient`. In-memory mock for deterministic unit tests.
    proof:
      kind: unit-test
      target: kimi-wire::client
      command: cargo test --lib -p kimi-wire
  - name: process_messages
    kind: fn
    visibility: pub
    contract: Re-exported from `kimi-wire::dispatch::process_messages`. Generic dispatch loop over `impl WireClient`.
    proof:
      kind: unit-test
      target: kimi-wire::dispatch
      command: cargo test --lib -p kimi-wire
  - name: WireMessage
    kind: enum
    visibility: pub
    contract: Re-exported from `kimi-wire::message::WireMessage`. Union of all incoming wire messages.
    proof:
      kind: unit-test
      target: kimi-wire::message
      command: cargo test --lib -p kimi-wire
  - name: WireResponse
    kind: struct
    visibility: pub
    contract: Re-exported from `kimi-wire::dispatch::WireResponse`. Response envelope sent back to the agent.
    proof:
      kind: unit-test
      target: kimi-wire::dispatch
      command: cargo test --lib -p kimi-wire
dependencies:
  internal: []
  external:
    - crate: kimi-wire
      path: ../kimi-wire
      reason: All wire protocol types, traits, and dispatch logic live in the external crate.
consumers:
  - path: runtime/wire_worker/task/process.rs
    uses: [ProcessWireClient, WireClientExt, WireMessage, RequestExt, EventExt]
  - path: runtime/scheduler/decompose.rs
    uses: [ProcessWireClient, WireClient, parse_wire_message, WireMessage, RequestExt]
invariants:
  - id: thin-layer
    rule: This module contains no logic; it only re-exports and provides OMK-specific compatibility aliases.
    proof:
      kind: inspection
      target: src/wire/mod.rs
  - id: generic-dispatch
    rule: process_messages is generic over WireClient, not bound to concrete type.
    proof:
      kind: unit-test
      target: kimi-wire::dispatch
      command: cargo test --lib -p kimi-wire
verification:
  pre_change:
    - cargo check --lib
  full:
    - cargo test --lib
    - cargo test --test mock_kimi_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# wire

## Architecture

`src/wire/` is a **thin re-export layer** over the external `kimi-wire` crate.
All protocol types, the `WireClient` trait, the dispatch loop, and extension
traits live in `kimi-wire`; OMK's module only preserves a few compatibility
aliases (`ProcessWireClient`, `ApprovalResponseType`, `redact_wire_secrets`)
so existing call sites keep compiling.

```
┌─────────────────────────────────────────┐
│  OMK crates (runtime, cli, mcp, ...)   │
│         use crate::wire::{...};         │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│      src/wire/mod.rs (re-exports)       │
│  pub use kimi_wire::{WireClient, ...}; │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│          kimi-wire crate                │
│  client, protocol, dispatch, message,   │
│  transport, redact, client_ext          │
└─────────────────────────────────────────┘
```

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Re-exports from `kimi-wire` + OMK compatibility aliases. |
| `README.md` | This file. |
| `AGENTS.md` | Editing rules for this module. |
| `TODO.md` | Known gaps / planned features. |

## Edit Rules

- **Do not add logic here.** Generic wire logic belongs in the `kimi-wire` crate.
- OMK-specific wire behaviour (e.g. worker auto-approval policy) belongs in the
  consumer module (`runtime/wire_worker/`), not in `src/wire/`.
- Check the official Kimi Wire docs before changing protocol fields or fallback
  behaviour in the upstream `kimi-wire` crate.
- Preserve compatibility aliases until all call sites are migrated.

## Tests

```bash
# Unit tests for the upstream crate
cargo test --lib -p kimi-wire

# Integration tests for OMK wire usage
cargo test --test mock_kimi_test
cargo test --test goal_cmd_test
```
