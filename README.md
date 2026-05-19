<div align="center">

<img src="assets/omk-kimi-hero.png" alt="oh-my-kimi banner" width="920">

# oh-my-kimi (`omk`)

**A Rust runtime that makes Kimi CLI production-grade.**

[![CI](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml)
[![Coverage](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/coverage.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/coverage.yml)
[![GitHub Release](https://img.shields.io/github/v/release/ekhodzitsky/oh-my-kimi?label=release&sort=semver)](https://github.com/ekhodzitsky/oh-my-kimi/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)

[Install](#install) · [Quick Start](#quick-start) · [Docs](#docs)

</div>

---

## Why?

Kimi CLI gives you answers. OMK gives you **runs** — tracked, verifiable, and reproducible.

- **Stop guessing what changed.** Every run leaves a `proof.json` you can inspect before merging.
- **Stop breaking your own code.** Parallel agents run in isolated git worktrees with automatic conflict detection.
- **Stop when it is not ready.** Verification gates, review walls, and honest `not_ready` / `blocked` terminal states — no silent failures shipped to `master`.
- **Stay local.** No cloud control plane, no hosted agents, no IDE lock-in. Your code never leaves your machine unless you push it.

## The One Command: `omk goal`

```bash
omk goal run "Add OAuth and rate-limiting to the API" --until-ready
```

OMK plans the work, builds a task graph, runs verification gates, dispatches bounded Kimi Wire agents in isolated worktrees, collects proof, and stops with an honest `ready` or `not_ready` verdict.

```bash
# See what happened
omk goal proof latest --format md
omk goal replay latest
```

**Status:** `omk goal` is a **Beta MVP**. Core flow works — planning, execution, verification, proof, slice isolation, and PR delivery. Budget hard stops, pause/resume, and crash recovery are in place. See [`TODO.md`](TODO.md) for what is next.

## What You Get

| Feature | What it does |
|---|---|
| **`omk goal`** | Proof-driven goal runner — plan, verify, execute, review, deliver |
| **Slice isolation** | Parallel feature work in git worktrees with conflict detection |
| **Verification gates** | Rust / Node / Python / Go presets + custom config |
| **Proof & replay** | Durable `proof.json`, `failure.json`, and event timelines |
| **Asset management** | Install / sync / doctor for Kimi agents, hooks, and skills |

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash
```

Or build from source (Rust 1.78+):

```bash
git clone https://github.com/ekhodzitsky/oh-my-kimi.git && cd oh-my-kimi
cargo build --release
```

macOS (arm64 / x86_64) and Linux x86_64. Windows is not supported yet.

## Quick Start

```bash
omk setup                 # create config, state, and data directories
omk doctor                # verify Kimi CLI and environment

omk goal run "Build a tiny Rust CLI with tests" --until-ready
omk goal proof latest
```

## Docs

- [`docs/TUTORIAL.md`](docs/TUTORIAL.md) — step-by-step first run
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — system design
- [`docs/API.md`](docs/API.md) — machine-readable CLI outputs
- [`ROADMAP.md`](ROADMAP.md) — where we are headed
- [`TODO.md`](TODO.md) — active backlog
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to contribute
- [`AGENTS.md`](AGENTS.md) — multi-agent workflow rules

## License

MIT © oh-my-kimi contributors
