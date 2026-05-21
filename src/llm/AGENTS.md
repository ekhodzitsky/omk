# src/llm/ Agent Guide

Module-specific rules for the LLM client & planner.

## Invariants

1. **No runtime::goal dependency.** This module must not import `runtime::goal::*`,
   `cli::goal::*`, task graph types, proof semantics, or worktree logic.
2. **Pure parsers.** Functions in `parser.rs` must be deterministic and side-effect
   free. No IO, no randomness, no mutable state.
3. **Private invariants.** `TokenBudget` fields are private; `used_tokens` must never
   exceed `max_tokens` (enforced via `saturating_add`).
4. **No unwrap in production.** All error paths return `LlmError`.

## Trait design notes

- `LlmClient` and `Planner` use `#[allow(async_fn_in_trait)]` instead of the
  `async_trait` crate. This makes them non-object-safe (`dyn LlmClient` is
  impossible). Consumers must use generics or `Arc<C>`.
- `WireLlmClient<W>` is generic over `W: WireClient + Send` because `WireClient`
  is not dyn-compatible (has generic methods and `async fn`).

## Ownership & lifecycle

- `WireLlmClient` borrows the wire client via `Arc<Mutex<W>>`. The caller owns
  `W` and must shut it down; `WireLlmClient` never spawns background tasks.
- `MockLlmClient` and `MockPlanner` are `Clone` via `Arc` internals and are safe
  to share across async tasks.

## Testing

- All public types must have unit tests in their source file under `#[cfg(test)]`.
- `MockLlmClient` and `MockPlanner` are the test doubles; no real Wire or HTTP
  calls in unit tests.
