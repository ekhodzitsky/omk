# omk v0.5.0: SOTA Hardened MVP

**omk** is a local, proof-driven autonomous engineering runtime powered by Kimi. Version 0.5.0 ships the hardened MVP with a full six-review wall, slice delivery, chat-first CLI, and durable goal state.

> 📦 Available on [crates.io](https://crates.io/crates/omk)

## What's New in 0.5.0

### Goal Delivery & Review
- **Full 6-review wall per slice delivery** — architect, code, test, security, performance, and anti-slop reviews run automatically before PR creation.
- **Auto-rebase with conflict recovery** — `attempt_auto_rebase` classifies merge conflicts as safe (whitespace, line-ending, comment-only) or unsafe and handles them accordingly.
- **Final merge gate with e2e GitHub validation** — `merge_policy` (`gated` / `manual` / `disabled`) fully enforced with CI polling and pre-flight checks.
- **Comprehensive recovery documentation** — see `docs/GOAL_RECOVERY.md` for all failure modes.

### Chat-First CLI
- **Chat-first surface** — running `omk` with no arguments opens a terminal-native chat REPL with conversation log, engine pane, and autonomous escalation.
- **Intent classifier** — routes requests by size and complexity using a heuristic layer backed by Kimi.
- **TUI pane rendering** — snapshot-tested terminal output with collapsed, compact, and expanded states.
- **Autonomous-mode default** — the router no longer blocks on preflight prompts unless explicitly opted in.

### MCP & Wire
- **MCP client commands** — `omk mcp list`, `omk mcp doctor`, and `omk mcp call` with stdio and HTTP/SSE transport support.
- **ApprovalProxy** — configurable approval policy engine (`Never`, `Safe`, `Yolo`, `Pattern`).
- **Wire hook integration** — native scripts placed in `.kimi/hooks/` execute automatically on hook requests.

### Goal State Hardening
- LLM planner in goal CLI with graceful heuristic fallback.
- Slice PR delivery hardening: auto-rebase, proof validation, and conflict detection.
- Durable `task-graph.json` with retry/lease metadata.
- Token/cost budget hard stops and recovery via `omk goal budget-add`.
- Deterministic `omk goal replay` output for crash-recovery inspection.
- Active pause interruption for Wire-backed agent waves.

## Installation

### Via cargo

```bash
cargo install omk
```

### From source

```bash
git clone https://github.com/ekhodzitsky/omk.git && cd omk
cargo build --release
```

### Via install script

```bash
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/omk/master/install.sh | bash
```

Also available via [Homebrew](homebrew/) and [AUR](aur/).

## Breaking Changes

No breaking changes for 0.x users. The `omk` binary name and CLI surface are stable.

## Migration Guide

If you previously installed from the `oh-my-kimi` repository name, GitHub will automatically redirect the old URLs. To update your local clone remote:

```bash
git remote set-url origin https://github.com/ekhodzitsky/omk.git
```

## Naming

This release completes the rebrand from **oh-my-kimi** to **omk**:
- Product name in prose and titles: **OMK**
- Binary, crate, and command references: `omk`

## Links

- crates.io: https://crates.io/crates/omk
- Repository: https://github.com/ekhodzitsky/omk
- Docs: https://github.com/ekhodzitsky/omk/tree/master/docs
