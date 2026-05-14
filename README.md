<div align="center">

<img src="assets/omk-kimi-hero.png" alt="oh-my-kimi wide banner with a blue Kimi mascot reaching out of the screen" width="920">

# oh-my-kimi (omk)

**Local, Wire-first, proof-driven orchestration for Kimi CLI**

Turn [Kimi CLI](https://github.com/MoonshotAI/kimi-cli) into observable local coding teams with role assets, scheduler state, verification gates, run timelines, and proof reports.

OMK is inspired by [oh-my-claudecode](https://github.com/yeachan-heo/oh-my-claudecode), but it is not a line-for-line port. It is a Kimi-native runtime: Kimi remains the execution engine, while OMK owns orchestration, state, recovery, evidence, and release-grade observability.

[![CI](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/ci.yml)
[![Coverage](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/coverage.yml/badge.svg?branch=master)](https://github.com/ekhodzitsky/oh-my-kimi/actions/workflows/coverage.yml)
[![GitHub Release](https://img.shields.io/github/v/release/ekhodzitsky/oh-my-kimi?label=github%20release&sort=semver)](https://github.com/ekhodzitsky/oh-my-kimi/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/MSRV-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Runtime: Wire-first](https://img.shields.io/badge/runtime-Wire--first-2563eb.svg)](#features)
[![Install: GitHub only](https://img.shields.io/badge/install-GitHub%20only-0ea5e9.svg)](#install)
[![crates.io](https://img.shields.io/badge/crates.io-not%20published-lightgrey.svg)](#install)
[![Status: beta MVP](https://img.shields.io/badge/status-beta%20MVP-0f172a.svg)](#mvp-status)

[Why](#why) - [MVP Status](#mvp-status) - [North Star](#north-star) - [Positioning](#positioning) - [Install](#install) - [First Run](#first-run) - [Workflow](#multi-agent-workflow) - [Features](#features) - [Commands](#commands) - [Why Better](#where-omk-is-stronger)

</div>

---

## Why

Kimi CLI is already useful as a single coding assistant. OMK is for the next step: when one chat window is not enough, and you want several local workers without losing control of what they are doing.

The core problem OMK solves is **trustworthy agent execution**. A useful coding run needs more than a confident final message. It needs visible workers, owned tasks, durable logs, failed-run artifacts, verification gates, and a proof you can inspect before you merge.

OMK wraps Kimi CLI with a local runtime that can:

- install Kimi-native agents, hooks, and skills into the current repo;
- run scheduler-backed Kimi teams through the Wire runtime;
- keep append-only event logs for worker, task, gate, and Wire evidence;
- detect ownership conflicts before parallel workers touch the same files;
- run verification gates and block "done" claims when required gates fail;
- render status through text, JSON, TUI, or web HUD surfaces;
- generate `proof.json` / `failure.json` artifacts for the final state of a run.

OMK is independent of Moonshot AI, Kimi CLI, and oh-my-claudecode.

## MVP Status

Short answer: **yes, you can use OMK today for local/personal repo automation, but treat it as a beta MVP, not a polished 1.0 product.**

Current source version: **v0.3.30**. We are intentionally **not publishing to crates.io yet**; install from GitHub release assets or from the GitHub repository.

What is ready enough to use now:

| Area | Readiness |
| --- | --- |
| GitHub release binaries | Ready for macOS arm64, macOS x86_64, and Linux x86_64. |
| `omk setup`, `omk doctor`, config/state paths | Ready. |
| Kimi asset sync/install/doctor/rollback | Ready for repo-local Kimi assets, manifests, dry-runs, drift checks, backups, and scoped output. |
| `omk team run` | Beta MVP. Scheduler-backed Wire runtime, writes events, proof, and failure artifacts. |
| Role packs | Ready: architect, executor, verifier, reviewer, integrator. |
| Run inspection | Ready: `omk run list`, `omk run show latest`, text/JSON output, filters. |
| Proof reports | Beta MVP: `omk proof show latest`, cached/regenerated proof, Markdown/text/JSON formats. |
| Verification gates | Ready for local gates and `.omk/gates.toml` customization, including full stdout/stderr evidence capture for large-output gates. |
| HUD | Text, JSON, and TUI are usable; web dashboard is still scaffold-level. |
| `omk goal` controller | Beta MVP. Creates durable goal state with PRD/plan/test/task/proof artifacts, classifies goal oracles, runs bounded Wire-backed agent waves, enforces budgets with `budget-add`, supports pause/resume/cancel, reruns gates after mutations, records review/security/integration evidence, renders PR drafts, and only marks `ready` when proof evidence passes. |
| Autopilot, Ralph, Ultrawork | Power-user MVP: useful, but less polished than the Kimi asset + team/proof path. |
| MCP server, marketplace, web dashboard | Secondary/scaffold surfaces. |

Current limits:

- No crates.io release yet.
- No Windows binary yet.
- Real non-mock runs require Kimi CLI installed and authenticated.
- Team workers run through Kimi Wire; there is no terminal-pane orchestration layer.
- Agent runs can edit your repository; use a clean git branch and inspect diffs.
- Some pre-1.0 command details may still change.

## North Star

`omk goal` is the main planned feature and the product direction for OMK:

```bash
omk goal run "Build or transform this project until it is proof-backed ready" --until-ready
```

The goal runtime is intended to plan, research, launch agents and subagents,
assign tasks, verify results, recover from failures, and stop only with a
truthful terminal status such as `ready`, `not_ready`, `blocked_on_human`, or
`needs_more_budget`.

The goal controller scaffold is in place. Today it:

- creates durable state under `goals/<goal-id>/` with safe loading for older records;
- writes planning artifacts (`prd.md`, `technical-plan.md`, `test-spec.md`,
  `task-graph.json`, `decisions.jsonl`) and an honest `proof.json`;
- blocks vague goals as `blocked_on_human` when no testable oracle exists;
- runs local verification gates with full evidence capture and post-mutation
  reruns when agents change project files;
- dispatches bounded Wire-backed agent waves with policy validation, per-task
  budget hard stops, stale-lease recovery, and `max-agents` worker-pool caps;
- accepts agent-proposed follow-up tasks under path-normalized read/write
  conflict policy, then dispatches them on later `execute` invocations;
- enforces wall-clock, token, and USD budgets with `needs_more_budget`, and
  supports operator recovery through `omk goal budget-add`;
- supports pause/resume/cancel with active worker interruption and
  deterministic event replay;
- records controller review, bounded secret-scan security evidence, and a
  structured six-pass review wall;
- supports explicit local integrator `accept` / `reject`, where `ready`
  requires gates, agent execution, review wall, integration, and oracle
  evidence;
- renders proof-backed PR title/body drafts through
  `omk goal open-pr latest --dry-run` without GitHub/network side effects.

Human PR merge/publish remains explicit and outside the automatic proof path.
The current `team run`, event log, gates, and proof systems remain the
execution foundation. The design lives in [SPEC.md](SPEC.md), the delivery path
in [ROADMAP.md](ROADMAP.md), the backlog in [TODO.md](TODO.md), and the
detailed design in
[docs/superpowers/specs/2026-05-11-omk-goal-design.md](docs/superpowers/specs/2026-05-11-omk-goal-design.md).

## Positioning

OMK is not trying to be a generic AI app builder, IDE autocomplete product, or
hosted coding-agent clone. The intended category is:

> Local, repo-native, proof-driven autonomous software engineering runtime.

The direct competitive set includes Devin, OpenHands, and Claude Code. Aider,
Dify, and Cody are adjacent benchmarks for terminal editing, agentic workflows,
and codebase context. OMK's wedge is trustable completion semantics: durable
goal state, bounded agents, verification gates, and inspectable proof artifacts.

The market map and wording rules are in
[Competitive Positioning](docs/COMPETITIVE_POSITIONING.md).

## Install

### Recommended: GitHub install script

```bash
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash
```

The script installs from GitHub. If Rust/Cargo is available, it uses:

```bash
cargo install --git https://github.com/ekhodzitsky/oh-my-kimi.git
```

Otherwise it downloads the matching binary archive from the latest GitHub Release.

### Manual GitHub release download

Download one of the latest GitHub release archives:

- macOS Apple Silicon: `omk-<version>-aarch64-apple-darwin.tar.gz`
- macOS Intel: `omk-<version>-x86_64-apple-darwin.tar.gz`
- Linux x86_64: `omk-<version>-x86_64-unknown-linux-gnu.tar.gz`

```bash
tar -xzf omk-<version>-<target>.tar.gz
chmod +x omk
./omk --help
```

Then move `omk` somewhere on your `PATH`, for example `~/.local/bin`.

### Build from source

```bash
git clone https://github.com/ekhodzitsky/oh-my-kimi.git
cd oh-my-kimi
cargo build --release
./target/release/omk --help
```

Requirements:

- Kimi CLI installed and authenticated for real agent runs.
- Rust 1.78+ for source builds and development.

## First Run

```bash
omk setup
omk doctor
omk kimi sync --dry-run
omk kimi sync
omk kimi doctor
```

`omk setup` creates config/state/data directories. `omk doctor` verifies the local environment. `omk kimi sync --dry-run` shows exactly which Kimi-native assets OMK would create or update before you write anything.

For a CI-safe demo with no real Kimi API calls:

```bash
MOCK_KIMI=1 ./scripts/north_star_demo.sh
```

## Multi-agent Workflow

OMK is built to be edited safely by multiple humans and agents (Codex, Kimi,
Claude, and future `omk goal` workers). The coordination contract is simple
and deliberately tracker-agnostic:

- `master` / `main` are **read-only** baselines. All work lands through a PR.
- Each task gets its own branch or worktree:
  `agent/<task-slug>`, `codex/<task-slug>`, `kimi/<task-slug>`,
  `claude/<task-slug>`.
- The PR declares task, owner, write scope, verification evidence, and known
  gaps. The PR body — not an external tracker — is the durable handoff surface.
- External trackers (Beads, GitHub Issues, Linear, …) are **optional** and must
  never become a hard prerequisite for building, testing, or reviewing OMK.

Bootstrap example:

```bash
git fetch origin
git worktree add ../omk-fix-foo -b claude/fix-foo origin/master
cd ../omk-fix-foo
# edit, run the verification wall, commit, push, open a PR
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development workflow and
[AGENTS.md](AGENTS.md) for the multi-agent hard constraints. The PR template
at [.github/pull_request_template.md](.github/pull_request_template.md) lists
the expected fields.

## Commands

### Kimi-native assets

```bash
omk kimi sync --dry-run
omk kimi sync
omk kimi install --dry-run
omk kimi doctor
omk kimi agents
omk kimi hooks
omk kimi skills
omk kimi rollback --dry-run
```

Use this path first if you want OMK's curated Kimi agents, hooks, and skills in a repository.

### Scheduler-backed team run

```bash
omk team run 3:executor "fix failing tests and produce a proof"
omk run show latest
omk proof show latest
omk hud --once
```

`omk team run` is the current proof-oriented path. It records events, dispatches tasks, runs verification gates, and writes a final proof or failure artifact.

### Run and proof inspection

```bash
omk run list
omk run show latest
omk run show latest --json
omk run show latest --worker worker-1
omk proof show latest
omk proof show latest --format json
omk proof show latest --regenerate
```

These commands are the main answer to "what actually happened?" after an agent run.

### Goal state scaffold

```bash
omk goal run "fix this repository until tests and proof pass" --until-ready
omk goal run "rewrite this service safely" --until-ready --budget-time 8h --budget-tokens 500000 --budget-usd 5
omk goal plan "prepare a migration proof plan"
omk goal list
omk goal status latest
omk goal show latest --format json
omk goal proof latest --format json
omk goal replay latest
omk goal replay latest --json
omk goal budget latest
omk goal budget latest --json
omk goal budget-add latest --tokens 50000 --usd 1.0
omk goal verify latest
omk goal execute latest
omk goal review latest
omk goal accept latest --summary "local integrator accepted the proof"
omk goal reject latest --reason "manual review found a blocker"
omk goal open-pr latest --dry-run --format markdown
omk goal open-pr latest --dry-run --format json
omk goal pause latest
omk goal resume latest
omk goal cancel latest
```

`omk goal` writes durable goal state plus planning, gate, agent, and review
evidence into `goals/<goal-id>/`. Proof artifacts stay honestly `not_ready`
until execution, review, oracle, and explicit integration evidence exist. When
run inside a git worktree, proofs also record best-effort git branch, HEAD
commit, and dirty-state. `omk goal open-pr` turns that proof into a dry-run PR
draft with status, readiness, task summary, delivery metadata, verification
wall, review/oracle/integration evidence, known gaps, changed files, and
artifacts.

## Features

| Feature | What it gives you | Status |
| --- | --- | --- |
| Kimi asset management | Sync/install/doctor/rollback for `.kimi/agents`, `.kimi/hooks`, and `.kimi/skills` with manifests and backups. | Current |
| Role packs | Curated Kimi-native roles for architecture, execution, verification, review, and integration. | Current |
| Scheduler-backed teams | Task claims, leases, retries, write-set conflict detection, event logs, and proof/failure artifacts. | Beta MVP |
| Wire protocol integration | Kimi Wire client, tolerant parser, mock fixture, and Wire evidence in run/proof output. | Current |
| Verification gates | Rust/Node/Python/Go presets, custom `.omk/gates.toml`, skipped gates, allow-fail gates, and robust captured stdout/stderr artifacts. | Current |
| Proof reports | `proof.json`, `failure.json`, cached/regenerated proof, readiness text, known gaps, and gate evidence. | Beta MVP |
| Run timelines | `events.jsonl` timeline, text/JSON output, worker/task/kind filters, malformed-line warnings. | Current |
| HUD | Text snapshots, JSON, TUI, and web dashboard scaffold. | Current/Scaffold |
| Cleanup and recovery | Team cleanup, backups, rollback, watchdog events, and interrupted-run failure artifacts. | Current |
| Goal runtime (`omk goal`) | Durable goal state, oracle-aware planning artifacts, bounded Wire-backed execution waves, post-mutation verification gates, review/security/integration evidence, task-scoped delivery metadata, deterministic replay, git evidence, PR dry-run rendering, and honest proof/failure artifacts. | Beta MVP |
| Autopilot | Single-lead autonomous execution with verification gates and resume/yolo options. | Power-user MVP |
| Ralph | Persistent verify/fix loop with iteration limits and completion evidence. | Power-user MVP |
| Ultrawork | Parallel burst prompts from args, files, or globs, with JSON output support. | Power-user MVP |

## Where OMK Is Stronger

Compared with raw Kimi CLI:

- OMK turns one-off prompts into tracked runs with state, tasks, gates, artifacts, and proof.
- OMK gives you local orchestration without hiding the underlying Kimi execution.
- OMK can show what workers did, not just what they concluded.

Compared with ad hoc shell scripts:

- OMK has typed event logs, run manifests, role packs, drift checks, rollback, and proof output.
- OMK records known gaps and failed gates instead of letting "done" mean "the command exited".
- OMK has CI-safe mock Kimi fixtures, so orchestration behavior can be tested without touching real Kimi config.

Compared with cloud agent orchestrators:

- OMK is local-first, Git-friendly, and explicit about files, commands, and verification evidence.
- OMK does not require a hosted control plane for the core workflow.
- OMK focuses on Kimi-native assets instead of treating Kimi as a generic interchangeable provider.

Compared with agentic coding assistants:

- OMK's north-star surface is a goal controller, not a chat session.
- OMK treats `ready`, `not_ready`, and `blocked` as evidence-backed terminal states.
- OMK's proof bundle is meant to survive after the assistant session ends.

## Development

```bash
git clone https://github.com/ekhodzitsky/oh-my-kimi.git
cd oh-my-kimi

cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
git diff --check
```

Useful local checks:

```bash
make repo-map
make wire-smoke      # requires local authenticated Kimi CLI
MOCK_KIMI=1 ./scripts/north_star_demo.sh
```

## Troubleshooting

| Issue | What to do |
| --- | --- |
| `kimi not found` | Install and authenticate Kimi CLI, then rerun `omk doctor`. |
| You are unsure what sync will change | Run `omk kimi sync --dry-run` first. |
| A run failed or looks stuck | Run `omk run show latest`, `omk proof show latest`, and `omk team health <team-name>`. |
| State looks stale | Use `omk team cleanup --dry-run` or `omk cleanup --teams --dry-run` before deleting anything. |

## More Docs

- [Tutorial](docs/TUTORIAL.md)
- [North Star tutorial](docs/north_star_tutorial.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Competitive Positioning](docs/COMPETITIVE_POSITIONING.md)
- [Roadmap](ROADMAP.md), [Spec](SPEC.md), [Goal backlog](TODO.md), [Contributing](CONTRIBUTING.md)

## License

MIT (c) oh-my-kimi contributors
