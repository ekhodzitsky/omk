<div align="center">

<img src="assets/omk-kimi-hero.png" alt="oh-my-kimi banner" width="920">

# oh-my-kimi (`omk`) — Archived

**Local, proof-driven autonomous engineering runtime powered by Kimi.**
*Archived as of June 2026. This repository is read-only.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)

</div>

---

## ⚠️ Why Archived

OMK was built around the **Kimi Wire Protocol** — a programmatic JSON-RPC interface that allowed external orchestrators to spawn, control, and intercept Kimi agents. This protocol enabled OMK's core capabilities:

- **Approval Proxy** — intercept and policy-gate every tool call
- **Lifecycle Hooks** — inject scripts at PreToolUse, Stop, and other checkpoints
- **Replay** — re-run a goal timeline from persisted events
- **Steer** — programmatically influence agent decisions

In mid-2026, MoonshotAI released **kimi-code** (v0.14.0), a TypeScript rewrite of the CLI that replaces the Wire Protocol with **ACP** (Agent Client Protocol). ACP is designed for IDE integration (editors like Zed and JetBrains driving a chat session), not for external orchestration.

**ACP does not provide:**
- Interception of tool calls (they auto-execute or use internal TUI flows)
- External hook injection
- Replay or steer methods
- Programmatic cancel of a running turn

Without these primitives, OMK cannot function as designed. A hotfix using `kimi-legacy` (the old Python CLI, v1.43.0) is possible but is a dead end — that CLI is no longer maintained.

The codebase remains a useful reference for several standalone components (see below).

---

## What OMK Was

OMK was an attempt to build a **local, proof-driven autonomous engineering runtime** — turning a high-level goal like *"Add OAuth and rate-limiting to the API"* into planned, verified, and delivered repository changes, entirely locally, with no cloud control plane.

Key differentiators that (at the time) no other tool offered:

| Capability | Description |
|---|---|
| **Multi-agent team scheduler** | Run N workers in parallel with role packs, task claims, leases, and inbox/outbox IPC |
| **Proof-driven delivery** | Every goal produced `proof.json` with gate results, readiness verdicts (`ready` / `not_ready` / `blocked`), and failure artifacts |
| **Durable goal state** | Goals survived process crashes. Pause, resume, cancel, replay from SQLite + JSONL state |
| **Git worktree isolation** | Parallel slice execution in isolated worktrees with conflict detection and auto-rebase |
| **Six-review wall** | Architect, code, test, security, performance, and anti-slop reviews before any PR |
| **Approval policy engine** | `Never` / `Safe` / `Yolo` / `Pattern` policies with timeouts and human-in-the-loop channels |
| **Cost tracking & budgets** | Token, USD, and time budgets with enforcement and reporting |
| **Verification gates** | Configurable gate runner (`cargo test`, `clippy`, `fmt`, security checks) with evidence capture |

---

## 🔧 Extractable Components

The following components from the OMK codebase can be reused as standalone utilities or libraries. Each links to the relevant crate or module in this repository.

| Component | Source | Description | Standalone Value |
|-----------|--------|-------------|-----------------|
| **Gate Runner** | [`crates/omk-gates/`](crates/omk-gates/), [`crates/omk-proof/`](crates/omk-proof/) | Configurable verification gate execution (`test`, `lint`, `fmt`, `security`) with structured `proof.json` output | Drop-in CI quality gate. Like `pre-commit` but with machine-readable artifacts |
| **Task Graph** | [`crates/omk-scheduler/`](crates/omk-scheduler/), [`crates/omk-db/`](crates/omk-db/) | Task dependency graph with claims, leases, retries, ownership, and SQLite persistence | Lightweight embedded alternative to Temporal or Airflow |
| **Event Log** | [`crates/omk-events/`](crates/omk-events/) | Append-only JSONL event envelopes with typed readers, writers, and sinks | Universal event-sourcing primitive for any Rust project |
| **Budget Tracker** | [`crates/omk-cost/`](crates/omk-cost/), [`crates/omk-db/`](crates/omk-db/) | Token/USD/time cost estimation and budget enforcement with SQLite storage | Attach to any LLM API client (OpenAI, Anthropic, etc.) |
| **Worktree Manager** | [`src/cli/team/worktree.rs`](src/cli/team/worktree.rs) (and related) | Git worktree creation, isolation, auto-rebase, and conflict detection | Parallel feature development without merge conflicts |
| **Approval Policy Engine** | [`crates/omk-wire-worker/src/approval/`](crates/omk-wire-worker/src/approval/) | Pattern-matching approval policies with timeouts and human channels | Can be adapted as a pre-hook for any agentic tool |

---

## 📚 Reference Documentation

The design and architecture documents remain valid references for building agent orchestration systems:

- [`SPEC.md`](SPEC.md) — Product spec, delivery contract, and goal lifecycle
- [`ROADMAP.md`](ROADMAP.md) — Staged roadmap (Stage 0–8) from foundation to long-horizon reliability
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — System design, module boundaries, and data flow
- [`docs/COMPETITIVE_POSITIONING.md`](docs/COMPETITIVE_POSITIONING.md) — Market map vs Devin, OpenHands, Claude Code, Aider
- [`AGENTS.md`](AGENTS.md) — Multi-agent workflow rules, Rust safety rules, async architecture, error handling doctrine
- [`docs/API.md`](docs/API.md) — Machine-readable CLI output contracts
- [`docs/TUTORIAL.md`](docs/TUTORIAL.md) — Step-by-step first run (historical reference)

---

## 🚀 Migration Paths

If you were using OMK or considering it, here are practical alternatives:

| Your Need | Alternative |
|---|---|
| Interactive AI coding in terminal | [**kimi-code**](https://github.com/MoonshotAI/kimi-code) — the official TypeScript CLI from MoonshotAI. Open source, MIT, ACP-based. |
| Multi-agent orchestration | [**OpenHands**](https://github.com/All-Hands-AI/OpenHands) or [**Devin**](https://devin.ai/) |
| Git worktree isolation | Use `git worktree` natively, or build on the reference logic in this repo |
| CI verification gates | Extract `omk-gates` logic, or use [`cargo-husky`](https://github.com/rhysd/cargo-husky) + custom JSON output |
| Cost tracking | Build on `omk-cost` crate, or use provider dashboards |

---

## 🤝 Contributing Ideas Upstream

**kimi-code** is open source (`github.com/MoonshotAI/kimi-code`, MIT, TypeScript). Several ideas explored in OMK may be valuable there:

- **Cost tracking & budgets** — token/USD visibility and enforcement per session
- **Approval policy engine** — pattern-based approvals with timeouts (beyond the current `default`/`auto`/`yolo` modes)
- **Structured proof artifacts** — a machine-readable completion report after each task
- **Verification gates** — optional post-task test/lint gate runner

If you are interested in carrying any of these forward, the OMK codebase is a fully working reference implementation.

---

## License

MIT © oh-my-kimi contributors
