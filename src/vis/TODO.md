# vis TODO

## Current
- [ ] Add contract-test or golden proof for `HudState::render_json` schema stability.
- [ ] Add integration test for `run_server` health endpoint (smoke test with axum TestClient).
- [ ] `hud_tui/mod.rs` uses `std::time::Instant` and `std::time::Duration` in async context; evaluate switching to `tokio::time` for consistency.
- [ ] `server/handlers.rs` uses `tokio::process::Command` without an explicit timeout (health check `kimi --version`).

## Next
- [ ] Extract server HTML into a build-time include or template to reduce `html.rs` line count.
- [ ] Add benchmark for `EventStream::poll` with large event files.
- [ ] Consider caching `TeamState::load` in server handlers to reduce disk I/O per request.
