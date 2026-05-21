# src/llm/ TODO

## Active

- [ ] Wire up `LlmUsage` tracking in consumers (caller must call `record_usage` on `TokenBudget` after each `complete`).
- [ ] Add HTTP-based `LlmClient` implementation (e.g. `ReqwestLlmClient`) for direct API access without Wire.
- [ ] Add integration tests under `tests/llm_integration_test.rs` that drive full `Planner → LlmClient → Parser` flow.

## Known gaps

- `WireLlmClient` does not handle tool calls or approval requests mid-stream.
  Non-interactive consumers should enable auto-approval upstream.
- `WireLlmClient` event loop does not filter events by request id; concurrent
  Wire activity could contaminate the response buffer.
- `CostEstimator::count_tokens` is CPU-bound BPE encoding. For very large
  prompts, consider `tokio::task::spawn_blocking`.
- `RetryPolicy` does not validate `base_delay <= max_delay`.

## Future PRs

- Integration into `runtime/goal/planner/` — replace heuristic oracle with `LlmPlanner`.
- Streaming response support (return `impl Stream<Item = String>` instead of `String`).
