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

[Why](#why) - [MVP Status](#mvp-status) - [North Star](#north-star) - [Positioning](#positioning) - [Install](#install) - [First Run](#first-run) - [Features](#features) - [Commands](#commands) - [Why Better](#where-omk-is-stronger)

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

Current source version: **v0.3.14**. We are intentionally **not publishing to crates.io yet**; install from GitHub release assets or from the GitHub repository.

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
| Verification gates | Ready for local gates and `.omk/gates.toml` customization. |
| HUD | Text, JSON, and TUI are usable; web dashboard is still scaffold-level. |
| `omk goal` controller scaffold | Current scaffold: creates durable goal state, planning artifacts, validated task graph, local verification task evidence, policy-validated multi-task Wire-backed agent task/mutation evidence, accepted and later-dispatched agent-proposed follow-up tasks with path-normalized dependency-ordered read/write access conflict policy, post-mutation gate reruns, controller review/security evidence, not-ready proof, and cancellation failure artifacts. |
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

The goal controller scaffold is implemented: it creates records under the OMK
state directory's `goals/` tree, writes `prd.md`, `technical-plan.md`,
`test-spec.md`, `task-graph.json`, and an honest `proof.json`, and supports
list/status/show/proof/verify/execute/review/cancel. The scaffold marks
controller-owned planning tasks as done with artifact evidence. `omk goal
verify` runs local verification gates and records gate evidence; `omk goal
execute` marks `goal-local-verify` done when required gates pass, turns
`goal-agent-execute` into a policy-validated multi-task Wire-backed wave,
records `task-policy.json`, per-task budgets, accepted/rejected task events,
outbox plus Wire event evidence, mutation diffs, and changed-file snapshots
under `artifacts/agent-runs/`. Wire workers may return structured
`OMK_TASK_PROPOSAL: {...}` follow-up work; the controller validates those
proposals, records `agent-task-proposals.json`, emits proposal/decision events,
appends accepted safe follow-up tasks as pending graph nodes, and emits
`task_graph_mutated` events for accepted graph additions. Task graphs are
validated on load for duplicate ids, missing dependencies, self-dependencies,
and dependency cycles before controller execution proceeds. Agent-proposed
follow-ups that write the same path, normalized alias path, parent/child path,
or read/write-overlapping path must be dependency-ordered; unordered access
conflicts are rejected with policy evidence instead of becoming graph nodes.
Later `execute`
invocations dispatch ready pending follow-ups through a
`goal-agent-followups` Wire wave and mark those durable graph nodes from worker
results. Agent waves now honor the goal `--max-agents` cap by creating a bounded
Wire worker pool for concurrently ready tasks, and expired leases are recovered
with `retry_scheduled` evidence that prefers another available worker over the
stale owner. When the agent changes project files, `execute` reruns verification
gates against the mutated tree and records post-mutation gate evidence. `omk
goal review` records controller review and bounded secret-scan security evidence
under `artifacts/reviews/`. Integration, specialist review loops, and ready
proof generation are still planned. The current `team run`, event log, gates, and
proof systems remain the execution foundation. The design is tracked in
[SPEC.md](SPEC.md), the delivery path in [ROADMAP.md](ROADMAP.md), and the task
backlog in [TODO.md](TODO.md).

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

### Team lifecycle

```bash
omk team roles
omk team run 2:executor "refactor the auth module"
omk team status <team-name>
omk team health <team-name>
omk team shutdown <team-name>
```

Use this when you want a named team state directory that can be inspected, health-checked, and shut down after a run.

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
omk goal plan "prepare a migration proof plan"
omk goal list
omk goal status latest
omk goal show latest --format json
omk goal proof latest --format json
omk goal verify latest
omk goal execute latest
omk goal review latest
omk goal cancel latest
```

`omk goal` currently creates durable goal state, planning artifacts, a task
graph with controller-owned task evidence, local verification task evidence,
local and post-mutation gate evidence, policy-validated multi-task Wire-backed
agent task/mutation evidence, accepted and later-dispatched agent-proposed follow-up tasks, controller review/security evidence, and honest
not-ready/cancelled proof artifacts. Goal proofs also capture best-effort git
branch, HEAD commit, and dirty-state evidence when run inside a git worktree.

### Power-user modes

```bash
omk autopilot "build a small REST API"
omk ralph --max-iterations 5 "make tests pass"
omk ultrawork --concurrency 4 "task one" "task two" "task three"
```

These modes are available and useful, but the strongest MVP path today is still: Kimi assets -> team run -> HUD/run/proof -> verification.

## Features

| Feature | What it gives you | Status |
| --- | --- | --- |
| Kimi asset management | Sync/install/doctor/rollback for `.kimi/agents`, `.kimi/hooks`, and `.kimi/skills` with manifests and backups. | Current |
| Role packs | Curated Kimi-native roles for architecture, execution, verification, review, and integration. | Current |
| Scheduler-backed teams | Task claims, leases, retries, write-set conflict detection, event logs, and proof/failure artifacts. | Beta MVP |
| Wire protocol integration | Kimi Wire client, tolerant parser, mock fixture, and Wire evidence in run/proof output. | Current |
| Verification gates | Rust/Node/Python/Go presets, custom `.omk/gates.toml`, skipped gates, allow-fail gates, and captured stdout/stderr. | Current |
| Proof reports | `proof.json`, `failure.json`, cached/regenerated proof, readiness text, known gaps, and gate evidence. | Beta MVP |
| Run timelines | `events.jsonl` timeline, text/JSON output, worker/task/kind filters, malformed-line warnings. | Current |
| HUD | Text snapshots, JSON, TUI, and web dashboard scaffold. | Current/Scaffold |
| Cleanup and recovery | Team cleanup, backups, rollback, watchdog events, and interrupted-run failure artifacts. | Current |
| Goal runtime | Durable goal state, plan/run/list/status/show/proof/verify/execute/review/cancel, planning artifacts, validated task graph with controller-owned, local verification, policy-validated multi-task Wire-backed agent mutation, accepted and later-dispatched agent-proposed follow-up tasks, path-normalized dependency-ordered read/write access conflict policy, `task_graph_mutated` events, post-mutation gate reruns, review, and security evidence, git evidence, local gate evidence, not-ready proof, and cancellation failure artifacts. Specialist reviews and integration loops are next. | Current Scaffold |
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
- [Project map](docs/PROJECT_MAP.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Competitive Positioning](docs/COMPETITIVE_POSITIONING.md)
- [Roadmap](ROADMAP.md)
- [Spec](SPEC.md)
- [Goal backlog](TODO.md)
- [Goal detailed design](docs/superpowers/specs/2026-05-11-omk-goal-design.md)
- [Contributing](CONTRIBUTING.md)

## License

MIT (c) oh-my-kimi contributors
