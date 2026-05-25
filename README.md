<div align="center">

<img src="assets/omk-kimi-hero.png" alt="OMK banner" width="920">

# OMK

**Local, proof-driven autonomous engineering runtime powered by Kimi.**
*Beta MVP (0.4.x, pre-1.0).*

[![CI](https://github.com/ekhodzitsky/omk/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/omk/actions/workflows/ci.yml)
[![Coverage](https://github.com/ekhodzitsky/omk/actions/workflows/coverage.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/omk/actions/workflows/coverage.yml)
[![GitHub Release](https://img.shields.io/github/v/release/ekhodzitsky/omk?label=release&sort=semver)](https://github.com/ekhodzitsky/omk/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)

[Install](#install) · [Quick Start](#quick-start) · [Docs](#docs)

</div>

---

## What is OMK?

OMK is a terminal-native autonomous agent that turns one high-level goal into planned, verified, and delivered repository changes. It runs entirely locally, requires no cloud control plane, and produces a `proof.json` artifact for every outcome.

- **Chat-first surface** — run `omk` to open a REPL. The classifier automatically escalates from quick answers through small edits to full goals. A visible engine pane shows what is happening under the hood.
- **Headless when you need it** — `omk goal run "..." --until-ready` drives planning, execution, verification, and review without blocking your terminal.
- **Proof-backed delivery** — every large goal decomposes into slices, passes a review wall, and lands as a PR with evidence. `ready`, `not_ready`, or `blocked` are the only terminal states.

## Install

**Fastest for Rust developers:**

```bash
cargo install omk
```

**macOS (Homebrew):**

```bash
brew install ekhodzitsky/oh-my-kimi/omk
```

**Universal installer:**

```bash
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/omk/master/install.sh | bash
```

**Build from source (Rust 1.78+):**

```bash
git clone https://github.com/ekhodzitsky/omk.git && cd omk
cargo build --release
```

Also available via [AUR](aur/).

Supported platforms: macOS (arm64 / x86_64) and Linux x86_64. Windows is not supported yet.

## Quick Start

```bash
omk setup                 # create config, state, and data directories
omk doctor                # verify Kimi CLI and environment

omk                       # open the chat REPL
# type a request and watch the engine pane (Tab to expand)
```

Run a goal headless:

```bash
omk goal run "Add OAuth and rate-limiting to the API" --until-ready
omk goal proof latest
omk goal replay latest
```

## How it works

**Single command surface.** `omk` with no arguments opens a chat REPL. There is no separate "agent mode" to learn. The same session handles quick questions, file edits, and large goals.

**Autonomous escalation.** A classifier routes every request to the right backend: trivial answers, small edits, medium plans, or large goals. You see progress in the engine pane and can expand it with Tab. The default autonomous mode does not block on confirmation dialogs.

**Proof-backed delivery.** Large goals become durable tasks with planning artifacts, verification gates, and bounded agent waves. The controller runs a review wall (architect, code, test, security, performance, anti-slop), records an honest `ready` or `not_ready` verdict, and writes `proof.json` as the artifact. Slices run in isolated git worktrees with conflict detection.

## What you can do

| Command | What it does |
|---|---|
| `omk` | Open the unified chat REPL (default) |
| `omk chat` | Open the unified chat REPL (alias `c`) |
| `omk goal run "..."` | Create a goal scaffold; add `--until-ready` to drive to completion |
| `omk goal plan "..."` | Create a plan scaffold without execution |
| `omk goal list` | List recorded goals |
| `omk goal status latest` | Compact status of the latest goal |
| `omk goal show latest` | Full goal state (add `--json` for machine output) |
| `omk goal proof latest` | Inspect the current proof artifact |
| `omk goal replay latest` | Replay the persisted goal timeline |
| `omk goal budget latest` | Show persisted budget checkpoints |
| `omk goal budget-add latest` | Extend an existing goal's budget |
| `omk goal verify latest` | Run local verification gates |
| `omk goal review latest` | Attach controller review evidence |
| `omk goal open-pr latest --dry-run --draft` | Render a PR draft from proof evidence |
| `omk goal accept latest --summary "..."` | Accept a proof-backed goal locally |
| `omk goal reject latest --reason "..."` | Reject a goal with a reason |
| `omk goal pause / resume / cancel latest` | Lifecycle controls |
| `omk goal merge latest` | Merge the GitHub PR for a ready goal |
| `omk mcp list` | List configured MCP servers |
| `omk mcp doctor` | Diagnose configured MCP servers |
| `omk mcp call <server> <tool>` | Call an MCP tool |
| `omk doctor` | Diagnose environment and dependencies |
| `omk setup` | Install hooks, skills, and config |
| `omk config show` | Show current config paths and values |
| `omk config validate` | Validate config.toml and environment |
| `omk config set` | Set a configuration value |

## Goal execution flags

`omk goal run` accepts flags that control budgets, agents, and delivery:

| Flag | Description |
|---|---|
| `--until-ready` | Drive plan, verify, execute, and review until ready or blocked |
| `--budget-time <DURATION>` | Wall-clock budget (for example `8h`, `7d`) |
| `--budget-tokens <N>` | Maximum estimated tokens |
| `--budget-usd <N>` | Maximum estimated USD cost |
| `--max-agents <N>` | Maximum number of agents |
| `--policy <POLICY>` | Delivery policy: `local`, `draft-pr`, `auto-pr` |
| `--merge-policy <POLICY>` | Merge policy: `disabled`, `manual`, `gated` |
| `--slice-execution` | Run agents in per-slice git worktrees |
| `--enforce-protection` | Enforce branch protection on main/master before integrator PR |
| `--no-llm-planner` | Disable the LLM planner; fall back to heuristic stub |
| `--planner-token-budget <N>` | Token budget per planner call (default: 8000) |

## Positioning

OMK is inspired by `oh-my-claudecode` and market-informed by the broader agentic coding landscape, including Devin, OpenHands, Claude Code, Aider, Dify, and Cody. It competes on durable goal state, explicit verification gates, and proof artifacts — not on feature breadth. See [`docs/COMPETITIVE_POSITIONING.md`](docs/COMPETITIVE_POSITIONING.md) for the full market map.

## Configuration

- Set `KIMI_API_KEY` in your environment.
- Config directory: `~/.config/omk/` (or `$XDG_CONFIG_HOME/omk/`).
- State directory: `~/.local/state/omk/` (or `$XDG_STATE_HOME/omk/`).
- Manage settings with `omk config show`, `validate`, and `set`.

## Roadmap & docs

- [`ROADMAP.md`](ROADMAP.md) — where we are headed
- [`SPEC.md`](SPEC.md) — product spec and delivery contract
- [`AGENTS.md`](AGENTS.md) — multi-agent workflow and contributing rules
- [`docs/TUTORIAL.md`](docs/TUTORIAL.md) — step-by-step first run
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — system design
- [`docs/API.md`](docs/API.md) — machine-readable CLI outputs

## License

MIT © omk contributors
