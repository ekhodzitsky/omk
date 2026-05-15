# notifications TODO

## Current
- [ ] Add unit tests for each platform payload formatter (Discord, Slack, Telegram) including truncation behavior.
- [ ] Add contract tests for `NotificationEvent` serde roundtrip with tagged serialization.
- [ ] Replace proxy re-exports in `webhook/mod.rs` with direct public module declarations or flatten the hierarchy.
- [ ] Add integration tests for `send_notification` using a mock HTTP server.

## Next
- [ ] Redact webhook URL tokens/credentials before they appear in logs or error contexts.
- [ ] Benchmark notification dispatch under concurrent event bursts.
