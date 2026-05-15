# mcp TODO

## Current
- [ ] Add unit/integration tests for JSON-RPC parse errors, request/response parity, and line-length capping.
- [ ] Add tests for each tool dispatch path (`omk_team_run`, `omk_team_status`, `omk_team_shutdown`, `omk_doctor`).
- [ ] Remove `#![allow(dead_code)]` from `mod.rs` and `server.rs` once all items have callers or tests.
- [ ] Extract stdio I/O behind a trait or port to enable testing without a real stdin/stdout.

## Next
- [ ] Add golden tests for JSON-RPC response shapes (initialize, tools/list, tools/call).
- [ ] Document MCP protocol version compatibility in contract proof.
