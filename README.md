<div align="center">

# 🌙 oh-my-kimi (omk)

**Multi-agent orchestration for [Kimi CLI](https://github.com/MoonshotAI/kimi-cli)**

*Inspired by [oh-my-claudecode](https://github.com/yeachan-heo/oh-my-claudecode) — reimagined for the Kimi ecosystem.*

[![CI](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml/badge.svg)](https://github.com/ekhodzitsky/oh-my-kimi/actions)
[![Release](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/release.yml/badge.svg)](https://github.com/ekhodzitsky/oh-my-kimi/releases)
[![Coverage](https://codecov.io/gh/ekhodzitsky/oh-my-kimi/branch/master/graph/badge.svg)](https://codecov.io/gh/ekhodzitsky/oh-my-kimi)
[![Crates.io](https://img.shields.io/crates/v/omk.svg)](https://crates.io/crates/omk)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](https://www.rust-lang.org)

</div>

---

> **Zero learning curve.** Don't learn Kimi CLI. Just use OMK.

`omk` turns Kimi CLI into a **multi-agent powerhouse**. Spawn teams of agents, run persistent execution loops, consult multiple AI providers, and manage everything from a single CLI — all through tmux panes and file-based IPC.

## ✨ Features

| Mode | What it does | Status |
|------|--------------|--------|
| 🚀 **Team** | Spawn N Kimi agents in tmux panes with shared task lists | ✅ Ready |
| 🤖 **Autopilot** | 6-phase autonomous execution (plan → execute → qa → validate) | 🚧 Active dev |
| 🔄 **Ralph** | Persistent verify/fix loops until every task is complete | 🚧 Active dev |
| 🧠 **Ask** | Cross-provider consultation (Claude, Codex, Gemini, Kimi) | 🚧 Active dev |
| 📊 **HUD** | Real-time tmux statusline + interactive TUI | ✅ Scaffold |
| 🔌 **MCP** | Model Context Protocol server for Cursor/Claude Desktop | 🚧 Scaffold |

## 🎬 Quick Start

```bash
# Install
 cargo install omk
# or
 curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash

# Setup
omk setup

# Spawn a team of 3 coder agents
omk team spawn 3:coder "refactor authentication to use JWT"

# Check progress
omk team status coder-a1b2

# Done
omk team shutdown coder-a1b2

# Shell completions
omk completions bash > ~/.local/share/bash-completion/completions/omk
omk completions zsh > ~/.zsh/completions/_omk
omk completions fish > ~/.config/fish/completions/omk.fish

# Man page
omk man > ~/.local/share/man/man1/omk.1
```

## 🏗️ Architecture

```
┌─────────┐     ┌─────────┐     ┌─────────┐
│  User   │────▶│   omk   │────▶│  tmux   │
└─────────┘     │  (Rust) │     └────┬────┘
                └─────────┘          │
                                     │
        ┌────────────────────────────┼────────────────────────────┐
        ▼                            ▼                            ▼
 ┌──────────────┐            ┌──────────────┐            ┌──────────────┐
 │  Lead Kimi   │            │ Worker Kimi  │            │ Worker Kimi  │
 │   (pane 0)   │            │   (pane 1)   │            │   (pane N)   │
 └──────┬───────┘            └──────┬───────┘            └──────┬───────┘
        │                           │                           │
        ▼                           ▼                           ▼
 ┌──────────────┐            ┌──────────────┐            ┌──────────────┐
 │ inbox.jsonl  │            │ inbox.jsonl  │            │ inbox.jsonl  │
 │ outbox.jsonl │            │ outbox.jsonl │            │ outbox.jsonl │
 │heartbeat.json│            │heartbeat.json│            │heartbeat.json│
 └──────────────┘            └──────────────┘            └──────────────┘
```

OMK is an **external orchestrator** — it does not fork or patch Kimi CLI. It spawns real `kimi` processes, coordinates them via JSONL files, and lets you attach to any session with standard tmux commands.

Read more in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## 📚 Commands

### Team Mode

```bash
# Spawn a team
omk team spawn 3:coder "fix all TypeScript errors"

# Check status (reads state + heartbeats + inbox/outbox counts)
omk team status <name>

# Graceful shutdown
omk team shutdown <name>
# Force kill
omk team shutdown <name> --force
```

### Autopilot

```bash
# Full autonomous pipeline
omk autopilot "build a REST API for task management"

# With persistence
omk autopilot --ralph "refactor the database layer"
```

### Ralph

```bash
# Persistent verify/fix loop
omk ralph "migrate from Express to Fastify"

# Limit iterations
omk ralph --max-iterations 5 "update all dependencies"
```

### Ask (Cross-Provider)

```bash
# Single advisor
omk ask claude "review my API design"

# Multi-advisor synthesis
omk ask all "architecture for real-time chat"
```

### HUD

```bash
# Tmux status bar
omk hud --tmux

# Interactive TUI
omk hud --tui

# Web dashboard
omk hud --web --port 8080

# Docker Compose
docker-compose up -d
# Open http://localhost:8080
```

### Diagnostics

```bash
# Check environment and dependencies
omk doctor

# Validate configuration
omk config validate

# Show current config
omk config show
```

### Maintenance

```bash
# Update omk to the latest release
omk update

# Clean up old state files
omk cleanup --older-than 7

# Remove all state (with confirmation)
omk cleanup --all

# Backup state
omk backup create
omk backup list
omk backup restore 20260507-121530

# Export/import state as JSON
omk state export --output my-state.json
omk state import --input my-state.json

# Manage skills
omk skill install https://github.com/user/omk-skill-repo
omk skill list
omk skill remove omk-skill-repo
```

### Shell Integration

```bash
# Generate completions
omk completions bash > ~/.local/share/bash-completion/completions/omk
omk completions zsh > ~/.zsh/completions/_omk
omk completions fish > ~/.config/fish/completions/omk.fish

# Generate man page
omk man > ~/.local/share/man/man1/omk.1
```

## 🚀 Getting Started

### 1. Install

```bash
cargo install omk
# or
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash
```

### 2. Verify

```bash
omk doctor
omk setup
```

### 3. Your first team

```bash
omk team spawn 3:coder "refactor auth to use JWT"
omk team status <name-from-output>
omk team shutdown <name>
```

### 4. Autopilot a feature

```bash
omk autopilot "build a REST API for task management"
# Resume if interrupted:
omk autopilot --resume --name ap-xxx "build a REST API"
```

### 5. Cross-consult advisors

```bash
omk ask all "review my database schema"
omk ask --providers claude,kimi "architecture for real-time chat"
```

### 6. Web dashboard

```bash
omk hud --web --port 8080
# Open http://localhost:8080
```

## 🧪 Development

```bash
git clone https://github.com/ekhodzitsky/oh-my-kimi
cd oh-my-kimi

# Run checks (fmt + clippy + test)
make check

# Build release binary
make release

# Install locally
make install
```

We follow **spec-driven development** and **TDD**. See [SPEC.md](SPEC.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## 🛠️ Troubleshooting

| Issue | Solution |
|-------|----------|
| `tmux not found` | Install tmux: `brew install tmux` / `apt install tmux` |
| `kimi not found` | Install [Kimi CLI](https://github.com/MoonshotAI/kimi-cli) |
| `omk team spawn` hangs | Check `omk doctor` — ensure tmux and kimi are available |
| State corruption | Use `omk cleanup --all` and re-run setup |
| Resume after crash | Use `--resume` flag with the run name |

## 📋 Roadmap

- [x] Team mode with tmux + JSONL IPC
- [x] Status & shutdown lifecycle
- [x] Skill injection system
- [x] TUI scaffold
- [x] XDG-compliant config paths
- [x] Atomic file writes + retry logic
- [x] State schema versioning + migration
- [x] Metrics collection
- [x] Multi-platform release CI
- [x] Shell completions + man page
- [x] Self-update
- [x] Environment diagnostics (`omk doctor`)
- [x] State cleanup (`omk cleanup`)
- [x] Config validation (`omk config`)
- [x] State backup/restore (`omk backup`)
- [x] State export/import (`omk state`)
- [x] Skill management (`omk skill`)
- [x] Autopilot 6-phase state machine with resume/yolo
- [x] Ralph persistence loop with resume/yolo
- [x] Cross-provider `ask` with synthesis
- [x] MCP server for IDE integration
- [x] Web dashboard (`omk hud --web`)
- [ ] Plugin marketplace

## 📄 License

MIT © oh-my-kimi contributors
