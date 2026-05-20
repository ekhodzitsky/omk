---
schema_version: "1.0"
module: llm
status: "active"
surface_api:
  - LlmClient
  - MockLlmClient
  - WireLlmClient
  - Planner
  - LlmPlanner
  - MockPlanner
  - CostEstimator
  - TokenBudget
  - RetryPolicy
  - LlmError
  - Plan
  - Slice
  - GoalClassification
  - GoalKind
  - Complexity
  - Difficulty
  - RepoContext
  - LlmResponse
  - LlmUsage
dependencies:
  - wire (read-only adapter)
  - cost (tiktoken-rs token counting pattern)
  - tokio (async runtime)
  - serde / serde_json (structured output)
  - thiserror (structured errors)
  - tracing (observability)
  - tiktoken-rs (token counting)
invariants:
  - "No dependency on runtime::goal, cli::goal, task graphs, or proof semantics"
  - "All public types implement Debug"
  - "No unwrap/expect in production code"
  - "WireLlmClient does not mutate src/wire/"
  - "Parser functions are pure and side-effect free"
verification_commands:
  - "cargo test --lib llm"
  - "cargo clippy --all-targets --all-features -- -D warnings"
  - "cargo doc --no-deps"
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
