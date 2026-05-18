# mcp — Agent Guide

## Editing Rules

1. **Registry owns the cache.** `McpRegistry` is the sole owner of per-server `moka`
   caches. Do not create caches outside `McpServerHandle`.
2. **Transport is a trait boundary.** `McpTransport` is the only way `McpRegistry`
   talks to servers. Do not leak `reqwest`, `tokio::process`, or SSE details past
   `McpClient`.
3. **Tool calls are idempotent-enough for caching.** The cache key is
   `(server_name, tool_name, serialized_args)`. If a tool has side effects,
   it must be documented and excluded from caching explicitly.
4. **No blocking in async.** `std::thread::sleep` and `std::sync::Mutex` are
   banned in all async paths under `src/mcp/`.
5. **Test through mocks.** `MockTransport` lives in `registry.rs` tests.
   All registry logic must be testable without spawning real processes.
