# cost — Agent Guide

## Editing Rules

1. **Estimator stays pure.** `estimator.rs` must not import `tokio::fs`,
   `std::path`, `crate::runtime`, or any other I/O. Only math and `serde`.
2. **Tracker knows no files.** `tracker.rs` works only through the `CostSink` trait.
   No direct calls to `atomic_write` or `tokio::fs` in tracker.
3. **New sinks are welcome.** If cost needs to go to a DB or webhook, add a new
   `xxx_sink.rs` with `impl CostSink for XxxSink`.
4. **Test through mocks.** All `CostTracker` logic is tested with `InMemoryCostSink`.
   Do not create temp files for tracker unit tests.
5. **PricingTier is the single source of truth.** If you change prices, change only
   `PricingTier::dollars_per_1m_tokens`.
