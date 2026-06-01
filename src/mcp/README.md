---
schema_version: 1
module: mcp
level: root
purpose: Expose an MCP (Model Context Protocol) server over stdio that dispatches JSON-RPC tool requests to OMK subcommands.
status: experimental
surface:
  - name: server
    kind: module
    visibility: pub
    contract: JSON-RPC stdio server implementation. Handles initialize, tools/list, and tools/call methods with length-capped line reading.
    proof:
      kind: missing
      target: src/mcp/server.rs
      command: ""
  - name: tools
    kind: module
    visibility: pub
    contract: Tool registry and async dispatch for MCP tool calls (team run, team status, team shutdown, doctor).
    proof:
      kind: missing
      target: src/mcp/tools.rs
      command: ""
  - name: run_mcp_server
    kind: fn
    visibility: pub
    contract: Starts the MCP stdio server loop, reading length-capped lines from stdin and writing JSON-RPC responses to stdout.
    proof:
      kind: missing
      target: src/mcp/server.rs run_mcp_server
      command: ""
  - name: list_tools
    kind: fn
    visibility: pub
    contract: Returns the static list of available MCP tool descriptors as JSON schema objects.
    proof:
      kind: missing
      target: src/mcp/tools.rs list_tools
      command: ""
  - name: handle_tool_call
    kind: fn
    visibility: pub
    contract: Dispatches a named MCP tool call with JSON arguments by spawning the local omk binary.
    proof:
      kind: missing
      target: src/mcp/tools.rs handle_tool_call
      command: ""
dependencies:
  internal:
    - module: error
      scope: OmkError for tool dispatch failures
      reason: Tool dispatch returns OmkError::InvalidInput and OmkError::ShellFailed.
  external:
    - name: anyhow
      scope: error handling in server.rs
      reason: Ergonomic Result propagation for async I/O.
    - name: serde / serde_json
      scope: JSON-RPC request/response serialization
      reason: Required by the MCP wire protocol.
    - name: tokio
      scope: async stdio I/O and process spawning
      reason: Server loop and tool dispatch are async.
    - name: tokio-stream / tokio-util
      scope: length-capped line framing
      reason: FramedRead with LinesCodec enforces MAX_MCP_LINE_LENGTH.
    - name: tracing
      scope: structured logging
      reason: Debug/warn/error logging for server events.
consumers:
  - path: src/cli/app/run.rs
    uses: ["run_mcp_server"]
  - path: src/wire/mod.rs
    uses: ["server"]
invariants:
  - id: line-length-cap
    rule: Stdio server reads lines capped at MAX_MCP_LINE_LENGTH (16 MiB) to prevent unbounded memory growth from hostile clients.
    proof:
      kind: static-check
      target: src/mcp/server.rs MAX_MCP_LINE_LENGTH
      command: "grep -n 'MAX_MCP_LINE_LENGTH' src/mcp/server.rs"
  - id: request-response-parity
    rule: Every inbound JSON-RPC request with an id receives a response; notifications receive none.
    proof:
      kind: missing
      target: src/mcp/server.rs handle_request
      command: ""
  - id: unknown-method-error
    rule: Unknown methods return JSON-RPC error code -32601 (Method not found).
    proof:
      kind: missing
      target: src/mcp/server.rs handle_request
      command: ""
  - id: tool-via-current-exe
    rule: Tool dispatch resolves the current executable path to invoke omk subcommands.
    proof:
      kind: missing
      target: src/mcp/tools.rs handle_tool_call
      command: ""
verification:
  pre_change:
    - cargo test --lib mcp
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# mcp

## Architecture

The `mcp` module implements a Model Context Protocol (MCP) server that communicates over standard input/output using JSON-RPC 2.0. It is designed to be invoked by MCP-compatible hosts (e.g., Claude Desktop, IDE plugins) that spawn the `omk` binary with a dedicated subcommand.

The module splits responsibility into two submodules:

- **`server`** — Owns the stdio transport, line framing, request parsing, and JSON-RPC response formatting. It enforces a 16 MiB line-length cap to prevent OOM from malicious or broken clients.
- **`tools`** — Owns the static tool registry (`list_tools`) and the async dispatch logic (`handle_tool_call`). Each tool is implemented by spawning the local `omk` binary with the appropriate CLI arguments and capturing stdout/stderr.

The root `mod.rs` acts as a storefront: it declares the two submodules and re-exports `run_mcp_server` for the CLI entrypoint.

## Files

| File | Responsibility |
|------|----------------|
| `mod.rs` | Module storefront; declares `server` and `tools`; re-exports `run_mcp_server`. |
| `server.rs` | JSON-RPC stdio server loop, request routing, and response serialization. |
| `tools.rs` | Tool descriptor registry and async dispatch via subprocess invocation. |
