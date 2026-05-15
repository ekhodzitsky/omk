# wire — TODO

## Current (pilot 2)
- [x] Extract `WireClient` trait.
- [x] Rename struct to `ProcessWireClient`.
- [x] Create `InMemoryWireClient` for unit tests.
- [x] Rewrite `wire/client/tests.rs` to use in-memory mock (6 of 8).

## Next
- [ ] Property-based tests for WireMessage serde roundtrip.
- [ ] Golden tests for new protocol version messages.
- [ ] Streaming `InMemoryWireClient` with `tokio::sync::mpsc` for async injection.
- [ ] `WireClientBuilder` for spawn parameter configuration.
- [ ] Wire Protocol v2 support (when Kimi CLI updates).
