---
schema_version: 1
module: llm
level: root
purpose: LLM client, planner, cost estimation, and structured-output parsing
status: experimental
surface:
  - name: LlmClient
    kind: trait
    visibility: pub
    contract: Async trait for LLM completion. Generic, non-object-safe.
    proof:
      kind: unit-test
      target: llm::client
      command: cargo test --lib llm::client
  - name: MockLlmClient
    kind: struct
    visibility: pub
    contract: In-memory test double for LlmClient.
    proof:
      kind: unit-test
      target: llm::client
      command: cargo test --lib llm::client
  - name: WireLlmClient
    kind: struct
    visibility: pub
    contract: Production implementation over the Wire protocol.
    proof:
      kind: unit-test
      target: llm::client
      command: cargo test --lib llm::client
  - name: Planner
    kind: trait
    visibility: pub
    contract: Trait for goal classification, decomposition, and estimation.
    proof:
      kind: unit-test
      target: llm::planner
      command: cargo test --lib llm::planner
  - name: LlmPlanner
    kind: struct
    visibility: pub
    contract: LLM-backed planner implementation.
    proof:
      kind: unit-test
      target: llm::planner
      command: cargo test --lib llm::planner
  - name: MockPlanner
    kind: struct
    visibility: pub
    contract: Configurable test planner.
    proof:
      kind: unit-test
      target: llm::planner
      command: cargo test --lib llm::planner
  - name: CostEstimator
    kind: struct
    visibility: pub
    contract: Token counting and USD estimation using tiktoken-rs.
    proof:
      kind: unit-test
      target: llm::cost
      command: cargo test --lib llm::cost
  - name: TokenBudget
    kind: struct
    visibility: pub
    contract: Tracks token consumption against a cap.
    proof:
      kind: unit-test
      target: llm::types
      command: cargo test --lib llm::types
  - name: RetryPolicy
    kind: struct
    visibility: pub
    contract: Exponential-backoff configuration.
    proof:
      kind: unit-test
      target: llm::retry
      command: cargo test --lib llm::retry
  - name: LlmError
    kind: enum
    visibility: pub
    contract: Structured error enum for LLM operations.
    proof:
      kind: unit-test
      target: llm::error
      command: cargo test --lib llm::error
dependencies:
  internal: []
  external: []
consumers:
  - path: runtime/goal/planner/scaffold/generate.rs
    uses: [Planner, RepoContext]
  - path: runtime/goal/mod.rs
    uses: [Planner]
invariants:
  - id: no-runtime-goal-dep
    rule: This module must not import runtime::goal, cli::goal, task graphs, or proof semantics.
    proof:
      kind: static-check
      target: llm module
      command: cargo check
  - id: pure-parsers
    rule: Parser functions in parser.rs must be deterministic and side-effect free. No IO, no randomness, no mutable state.
    proof:
      kind: unit-test
      target: llm::parser
      command: cargo test --lib llm::parser
  - id: no-unwrap-production
    rule: All error paths return LlmError. No unwrap/expect/panic in production code.
    proof:
      kind: static-check
      target: llm module
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib llm
    - cargo clippy --all-targets --all-features -- -D warnings
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
    - cargo doc --no-deps
---

# src/llm/ — LLM Client & Planner

Self-contained module for LLM calls and goal planning.

## Purpose

`src/llm/` abstracts away prompt engineering, structured-output parsing, retry
logic, token budgets, and cost estimation.  It exposes typed traits
([`LlmClient`], [`Planner`]) that any consumer — including future
`runtime/goal/planner/` integration — can use without knowing whether the
backend is Kimi Wire, HTTP, or an in-memory mock.

## File map

| File | Responsibility |
|------|----------------|
| `mod.rs` | Public re-exports |
| `client.rs` | `LlmClient` trait, `MockLlmClient`, `WireLlmClient` |
| `planner.rs` | `Planner` trait, `LlmPlanner`, `MockPlanner` |
| `cost.rs` | `CostEstimator` with tiktoken-rs |
| `retry.rs` | `RetryPolicy` and exponential-backoff helper |
| `parser.rs` | Structured JSON parsers (Plan, Classification, Criteria) |
| `prompt.rs` | Deterministic prompt templates with versioning |
| `error.rs` | `LlmError` enum |
| `types.rs` | Domain types: `Plan`, `Slice`, `GoalKind`, etc. |
| `README.md` | This file |

## Architecture

```
Consumer (runtime/goal/ — future PR)
  │
  ├─ LlmClient::complete(prompt, budget)
  ├─ Planner::classify(goal)
  ├─ Planner::decompose(goal, context)
  └─ CostEstimator::estimate(tokens, model)
  │
  ▼
src/llm/
  ├─ LlmClient trait
  ├─ WireLlmClient (wraps existing Wire client)
  ├─ MockLlmClient (test double)
  ├─ Planner trait
  ├─ LlmPlanner (prompt → LLM → parser)
  ├─ MockPlanner (configurable test double)
  └─ CostEstimator, RetryPolicy, parser functions
  │
  ▼
Kimi CLI (wire) or HTTP API
```

## Status

- **Phase 1 (this PR)**: Module created with full trait contracts, tests, and documentation.
- **Phase 2 (future PR)**: Integration into `runtime/goal/planner/`.

## Known gaps

- `WireLlmClient` reads the Wire event stream but does not yet handle
  approval requests or tool calls that may interrupt the prompt flow.
  Non-interactive workers should configure auto-approval upstream.
- HTTP-based `LlmClient` implementation is not yet provided; add a new
  struct implementing `LlmClient` if direct API access is needed.
