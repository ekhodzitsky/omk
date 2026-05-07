# oh-my-kimi (omk)

Multi-agent orchestration for [Kimi CLI](https://github.com/MoonshotAI/kimi-cli). Inspired by [oh-my-claudecode](https://github.com/yeachan-heo/oh-my-claudecode).

> **Zero learning curve.** Don't learn Kimi CLI. Just use OMK.

## Features

- **Team Mode** — Spawn N Kimi agents in tmux panes with file-based IPC
- **Autopilot** — 6-phase autonomous execution pipeline
- **Ralph** — Persistent mode with verify/fix loops until complete
- **Skills System** — Portable `SKILL.md` format (compatible with Claude Code skills)
- **Cross-Provider Ask** — Consult Claude, Codex, Gemini, or Kimi and synthesize answers
- **HUD / TUI** — Real-time statusline and terminal UI

## Requirements

- [Kimi CLI](https://github.com/MoonshotAI/kimi-cli) installed and authenticated
- [tmux](https://github.com/tmux/tmux)
- Rust 1.78+ (for building from source)

## Quick Start

```bash
# Install
cargo install --path .

# Setup
omk setup

# Run a team of 3 coder agents
omk team 3:coder "fix all TypeScript errors"

# Autonomous execution
omk autopilot "build a REST API for task management"

# Persistent mode
omk ralph "refactor authentication module"
```

## Architecture

OMK is an **external orchestrator** — it does not fork or patch Kimi CLI. Instead, it:

1. Spawns multiple `kimi` processes in tmux panes
2. Injects orchestration prompts and skills
3. Coordinates agents via JSONL inboxes/outboxes
4. Observes state through wire files and heartbeat JSON

```
┌─────────────────┐
│   omk team      │
│  (orchestrator) │
└────────┬────────┘
         │
    ┌────┴────┐
    │  tmux   │
    └────┬────┘
   ┌─────┼─────┐
   ▼     ▼     ▼
┌────┐┌────┐┌────┐
│lead││w0  ││w1  │
│kimi││kimi││kimi│
└────┘└────┘└────┘
```

## Project Structure

```
oh-my-kimi/
├── src/
│   ├── cli/          # Subcommands: team, autopilot, ralph, ask, hud
│   ├── runtime/      # tmux IPC, bridge, state machine, worker lifecycle
│   ├── skills/       # Skill discovery, parser, injector
│   ├── vis/          # HUD / TUI
│   └── mcp/          # MCP server (future)
├── skills/           # Bundled skills (team, autopilot, ralph, ultrawork)
├── agents/           # Agent prompt definitions
└── hooks/            # Shell hook templates for Kimi CLI
```

## Status

This project is in **early MVP** stage. Team mode is functional; autopilot and ralph are scaffolded.

## License

MIT
