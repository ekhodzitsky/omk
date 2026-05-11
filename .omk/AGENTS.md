---
name: oh-my-kimi
description: Orchestration layer for Kimi CLI — team mode, event logs, proof generation, and scheduler-backed execution
agents:
  - role: architect
    description: Designs system structure, APIs, and runtime scheduler
    tier: lead
  - role: executor
    description: Implements features, writes tests, fixes bugs
    tier: worker
  - role: verifier
    description: Runs gates, checks proofs, validates completeness
    tier: qa
  - role: reviewer
    description: Reviews code, docs, and design decisions
    tier: qa
  - role: integrator
    description: Merges branches, resolves conflicts, prepares releases
    tier: ops
---

# oh-my-kimi Agent Context

## Official Documentation

We actively use the official Kimi Code CLI documentation:

- **Main docs**: https://www.kimi.com/code/docs
- **Wire Protocol**: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html
- **Kimi Agent (Rust)**: https://github.com/MoonshotAI/kimi-agent-rs — lightweight Wire-only Rust implementation

## Wire Protocol Reference

Kimi Code CLI supports `--wire` mode for structured bidirectional communication via JSON-RPC 2.0 over stdin/stdout. Protocol version: 1.9.
The code anchor is `src/wire/protocol/mod.rs::KIMI_WIRE_PROTOCOL_VERSION`; update this file, README/docs, changelog, and tests whenever that constant or the observed protocol shape changes.

### Initialization

```json
// Client → Agent
{"jsonrpc": "2.0", "method": "initialize", "id": "1", "params": {
  "protocol_version": "1.9",
  "client": {"name": "omk", "version": "0.3.1"},
  "capabilities": {"supports_question": true, "supports_plan_mode": true},
  "hooks": [
    {"id": "pre-tool", "event": "PreToolUse", "matcher": "Shell", "timeout": 30}
  ]
}}

// Agent → Client
{"jsonrpc": "2.0", "id": "1", "result": {
  "protocol_version": "1.9",
  "server": {"name": "Kimi Code CLI", "version": "1.41.0"},
  "slash_commands": [{"name": "init", "description": "Analyze codebase", "aliases": []}],
  "capabilities": {"supports_question": true},
  "hooks": {"supported_events": ["PreToolUse", "Stop"], "configured": {"PreToolUse": 1}}
}}
```

### Prompt Flow

```json
// Client → Agent
{"jsonrpc": "2.0", "method": "prompt", "id": "2", "params": {"user_input": "Fix failing tests"}}

// Agent → Client (events during turn)
{"jsonrpc": "2.0", "method": "event", "params": {"type": "TurnBegin", "payload": {"user_input": "Fix failing tests"}}}
{"jsonrpc": "2.0", "method": "event", "params": {"type": "StepBegin", "payload": {"n": 1}}}
{"jsonrpc": "2.0", "method": "event", "params": {"type": "ContentPart", "payload": {"type": "text", "text": "I'll help you fix the tests."}}}
{"jsonrpc": "2.0", "method": "request", "id": "req-1", "params": {"type": "ApprovalRequest", "payload": {"id": "app-1", "tool_call_id": "tc-1", "sender": "Shell", "action": "run shell command", "description": "cargo test"}}}

// Client → Agent (approval response)
{"jsonrpc": "2.0", "id": "req-1", "result": {"request_id": "app-1", "response": "approve"}}

// Agent → Client (turn complete)
{"jsonrpc": "2.0", "method": "event", "params": {"type": "TurnEnd", "payload": {}}}
{"jsonrpc": "2.0", "id": "2", "result": {"status": "finished"}}
```

### All Event Types

| Event | When | Key Fields |
|-------|------|------------|
| `TurnBegin` | Turn starts | `user_input` |
| `TurnEnd` | Turn ends | — |
| `StepBegin` | Step starts | `n` (step number) |
| `StepInterrupted` | Step interrupted | — |
| `CompactionBegin/End` | Context compaction | — |
| `StatusUpdate` | Stats update | `context_usage`, `context_tokens`, `plan_mode` |
| `ContentPart` | AI output | `type`: text/think/image_url/audio_url/video_url |
| `ToolCall` | Tool invoked | `id`, `function.name`, `function.arguments` |
| `ToolCallPart` | Streaming args | `arguments_part` |
| `ToolResult` | Tool done | `tool_call_id`, `return_value` |
| `ApprovalResponse` | Approval done | `request_id`, `response` |
| `SubagentEvent` | Subagent msg | `parent_tool_call_id`, `event` |
| `SteerInput` | Input appended | `user_input` |
| `PlanDisplay` | Plan shown | `content`, `file_path` |
| `HookTriggered` | Hook starts | `event`, `target`, `hook_count` |
| `HookResolved` | Hook ends | `event`, `target`, `action`, `reason` |

### All Request Types

| Request | When | Response |
|---------|------|----------|
| `ApprovalRequest` | Tool needs approval | `ApprovalResponse` |
| `ToolCallRequest` | External tool call | `ToolResult` |
| `QuestionRequest` | `AskUserQuestion` tool | `QuestionResponse` |
| `HookRequest` | Hook execution | `HookResponse` |

### Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 | Turn in progress / not supported |
| -32001 | LLM not configured |
| -32002 | LLM not supported |
| -32003 | LLM service error |

## Kimi Agent (Rust)

Experimental Rust implementation: `MoonshotAI/kimi-agent-rs`
- Wire mode only, single static binary
- Same config (`~/.kimi/config.toml`)
- Limitations: Kimi-only, no login, no `--prompt`, no SSH
- Binary: `kimi-agent` (replaces `kimi --wire`)

## OMK Wire Integration Roadmap

- [x] Wire protocol type definitions (`src/wire/protocol.rs`)
- [x] Wire client scaffold (`src/wire/client.rs`)
- [x] Wire-backed team runner (spawn workers via wire instead of tmux)
- [x] Event bridge (wire events -> OMK events.jsonl)
- [ ] Approval proxy (OMK approves/rejects on behalf of user)
- [ ] Hook integration (OMK hooks via wire HookRequest)

## Project Conventions

- Kimi-only first; provider-neutral workers are deferred
- Event-driven: all team operations emit typed events to `events.jsonl`
- Proof-first: every run produces a `Proof` with gates, changed files, failures, known gaps
- Scheduler-backed: `ClaimStore` + `OwnershipMap` + `RunManifest` for task lifecycle
- Wire protocol changes require serde roundtrip/golden tests, unknown/extra field behavior when relevant, and redaction tests for secret-like fields.
- Machine-readable streams stay clean: do not mix human logs into Wire stdout, JSONL events, proof JSON, or MCP JSON payloads.
- Worker lifecycle is explicit: every Wire worker, spawned task, and child process needs a cancellation, join/abort, and event/proof evidence path.

## Rust Safety Rules (Hard Constraints)

These rules apply to **all production code** under `src/` (outside `#[cfg(test)]`).
Violations must be fixed before merge.

1. **`unwrap()` is banned.** Use `?`, `if let`, `match`, `ok_or`, `bail!`, or `.context()`.
2. **`expect()` is banned.** No "this should never happen" — it always happens eventually.
3. **`panic!()` is banned.** Graceful degradation only; propagate errors via `Result`.
4. **`std::thread::sleep` is banned in `async fn`.** Use `tokio::time::sleep(...).await`.
5. **`std::sync::Mutex` is banned in `async fn`.** Use `tokio::sync::Mutex` to avoid blocking the executor.
6. **All external `Command::output().await` must have a `tokio::time::timeout`.** Prevent infinite hangs from rogue child processes.
7. **All `spawn()` calls must set `kill_on_drop(true)` or attach to a `CancellationToken`.** Prevent zombie processes.

### Tests (`#[cfg(test)]`)

`unwrap()`/`expect()` are allowed for brevity, but prefer `?` where it keeps the test readable.
