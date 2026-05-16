<div align="center">

<img src="assets/omk-kimi-hero.png" alt="oh-my-kimi wide banner with a blue Kimi mascot reaching out of the screen" width="920">

# oh-my-kimi (omk)

**Local, repo-native, proof-driven orchestration for Kimi CLI.**

Turn a single chat window into an observable team of local coding workers — with role assets, scheduler state, verification gates, run timelines, and proof you can inspect before merging.

[![CI](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml)
[![Coverage](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/coverage.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/coverage.yml)
[![GitHub Release](https://img.shields.io/github/v/release/ekhodzitsky/oh-my-kimi?label=release&sort=semver)](https://github.com/ekhodzitsky/oh-my-kimi/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/MSRV-1.78%2B-orange.svg)](https://www.rust-lang.org)

[Install](#install) · [First Run](#first-run) · [Features](#features) · [Docs](#docs)

</div>

---

## What & Why

Kimi CLI is great for one-off answers. OMK is for when you want **repeatable, verifiable agent runs** across your repository — without losing control of what changed, why, and whether it actually works.

OMK wraps Kimi CLI with a local runtime that installs native agents, dispatches scheduler-backed teams, detects file conflicts before parallel workers collide, runs verification gates, and writes durable `proof.json` artifacts. It is independent of Moonshot AI and not a generic app builder. The category is simple:

> *Local, repo-native, proof-driven autonomous software engineering runtime.*

## Install

```bash
# One-liner from GitHub
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash

# Or build from source
git clone https://github.com/ekhodzitsky/oh-my-kimi.git && cd oh-my-kimi
cargo build --release
```

Binaries are available for macOS (arm64 / x86_64) and Linux x86_64. Windows is not supported yet. We do not publish to crates.io yet — GitHub releases and source builds are the canonical paths.

## First Run

```bash
omk setup                 # create config, state, and data directories
omk doctor                # verify Kimi CLI and local environment

# Run a goal with proof-backed readiness
omk goal run "Build a tiny Rust CLI with tests and proof evidence" --until-ready

# Or run with concurrent slices in isolated worktrees
omk goal run "Add OAuth + rate-limiting to the API" \
  --until-ready --slice-execution --delivery draft-pr --merge-policy gated

# Inspect what happened
omk goal proof latest --format md
omk goal replay latest
```

`--slice-execution` decomposes the goal into independent features, runs each in its own git worktree and branch, opens per-slice PRs, runs review/fix loops, and finally creates an integrator PR with reran gates. The CLI renders a live narrative with step icons so you can watch the orchestrator work.

## What You Get

- **🧩 Kimi-native assets** — sync/install/doctor/rollback for `.kimi/agents`, hooks, and skills with manifests and backups.
- **👥 Scheduler-backed teams** — task claims, leases, retries, write-set conflict detection, event logs, and proof/failure artifacts.
- **🧪 Verification gates** — Rust/Node/Python/Go presets plus custom `.omk/gates.toml`, with full stdout/stderr capture.
- **🎯 Goal runtime (`omk goal`)** — durable state, oracle-aware planning, bounded Wire-backed execution waves, optional concurrent slice isolation, post-mutation gate reruns, per-slice PR delivery, integrator merge, budget hard stops, pause/resume, and honest `proof.json`/`failure.json` artifacts.
- **📊 Observability** — text/JSON/TUI HUD, run timelines, worker health, and deterministic replay.

## Features

| Surface | What it does | Status |
|---|---|---|
| Kimi asset management | Sync/install/doctor/rollback for agents, hooks, skills | Ready |
| Role packs | Architect, executor, verifier, reviewer, integrator | Ready |
| Scheduler-backed teams | Task dispatch, conflict detection, event logs, proof | Beta MVP |
| Verification gates | Presets + custom config, stdout/stderr capture | Ready |
| Proof reports | `proof.json`, `failure.json`, text/JSON/Markdown | Beta MVP |
| Goal runtime | Plan → verify → execute → review → deliver → integrate | Beta MVP |
| HUD / timelines | Text, JSON, TUI, web scaffold | Ready / Scaffold |

## Where OMK Is Stronger

| vs | The difference |
|---|---|
| **Raw Kimi CLI** | OMK turns one-off prompts into tracked runs with tasks, gates, artifacts, and proof. |
| **Ad hoc scripts** | Typed event logs, run manifests, role packs, drift checks, rollback, and honest failure artifacts. |
| **Cloud orchestrators** | Local-first, Git-friendly, no hosted control plane, Kimi-native instead of generic. |
| **Chat assistants** | Goal controller with `ready` / `not_ready` / `blocked` as evidence-backed terminal states. |

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) for the multi-agent workflow and hard constraints.

## Docs

- [Tutorial](docs/TUTORIAL.md)
- [North Star tutorial](docs/north_star_tutorial.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Competitive Positioning](docs/COMPETITIVE_POSITIONING.md)
- [Roadmap](ROADMAP.md), [Spec](SPEC.md), [Backlog](TODO.md)

## License

MIT © oh-my-kimi contributors
