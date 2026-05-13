# OMK Project Map

This file is the first stop for AI agents working on oh-my-kimi. It keeps the project cheap to navigate as the codebase grows.

## Start Here

1. Read this map.
2. Read the README for the area you will touch:
   - `src/cli/README.md`
   - `src/runtime/README.md`
   - `src/wire/README.md`
   - `src/kimi_native/README.md`
3. Run `scripts/repo-map.sh` when you need a fresh code index.
4. Use Kimi `explore` for read-only lookup before broad edits, then use `plan` or `coder` only after the target files are known.

Do not split or reorganize the large files just because they are large. File-splitting is a separate refactor and must be requested explicitly.

## Product Shape

OMK is a Rust orchestration runtime for Kimi CLI. It keeps Kimi as the execution engine while OMK owns process control, state, scheduling, retries, visibility, and proof artifacts.

The current public wedge is Kimi-only:

```text
omk kimi sync
omk team run 2:coder "fix all failing tests and produce a proof"
omk hud --once
omk proof show latest
```

The core commands are current; web HUD, secondary dashboard surfaces, and proof/operator ergonomics are still hardening. `omk goal` has a current state-core scaffold plus load-time task graph validation, first-class graph mutation events, policy-validated multi-task Wire-backed execution waves with mutation evidence, accepted and later-dispatched agent-proposed follow-up tasks, bounded `max_agents` worker pools, stale-lease recovery evidence, post-mutation gate reruns, and controller review/security evidence, and is the planned north-star workflow for long-running proof-backed engineering goals. Check `README.md`, `SPEC.md`, `ROADMAP.md`, and `TODO.md` before promising a command is fully polished.

Competitive positioning is tracked in `docs/COMPETITIVE_POSITIONING.md`. Keep
public wording anchored on "local, repo-native, proof-driven autonomous software
engineering runtime" rather than "Devin clone" or "generic agent workflow app."

## Current vs Target (L8) Snapshot

- **Current:** Kimi-only execution, scheduler-backed Wire `team run`, run/proof inspection, and Kimi-native asset sync/doctor/rollback.
- **Current Scaffold:** `omk goal` controller scaffold with controller-owned task evidence, local verification task evidence, load-time task graph validation, first-class graph mutation events, policy-validated multi-task Wire-backed agent execution with mutation evidence, accepted and later-dispatched agent-proposed follow-up tasks, bounded `max_agents` worker pools, stale-lease recovery evidence, post-mutation gate reruns, controller review/security evidence, git evidence, and local gate evidence; web HUD and secondary dashboard/MCP surfaces are present but still hardening.
- **Target:** `omk goal` as the proof-first controller that plans, researches, spawns agents, verifies, and stops with a truthful terminal status.

When writing docs or implementation notes, mark maturity explicitly (`Current`, `Current MVP`, `Current Scaffold`, `Next`, `Later`) to avoid mixing shipped behavior with roadmap intent.

## Official Kimi Contract

Use official Kimi Code docs as the source of truth before changing Kimi integration:

- Docs root: https://www.kimi.com/code/docs
- Wire Protocol: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html
- Skills: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills.html
- Subagents: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/sub-agents.html

Default rule: new Kimi process-control work is Wire-first. Prefer `kimi --wire` events and requests over prompt scraping.
Protocol version details can change; treat [KIMI_UPSTREAM.md](KIMI_UPSTREAM.md) + fresh `kimi info`/upstream docs re-check as source of truth instead of hardcoding a version in prose.
Legacy/no-handshake fallback remains valid when upstream does not support `initialize`.

## Repository Areas

| Area | Purpose | Start With | Common Tests |
| --- | --- | --- | --- |
| `src/main.rs` | Thin binary wrapper that calls `omk::cli::run()`. | `src/cli/README.md` | `tests/library_api_test.rs` |
| `src/cli/` | Clap app, command handlers, and user-facing command behavior compiled through the library crate. | `src/cli/README.md` | `tests/*_test.rs` matching the command |
| `src/cli/team/` | Focused helpers for team proof artifacts and Wire run support. | `src/cli/README.md` | `cargo test finalize_team_run_proof`, `tests/team_lifecycle_test.rs` |
| `src/runtime/` | State, process control, scheduler, events, proof, watchdogs. | `src/runtime/README.md` | `tests/team_lifecycle_test.rs`, `tests/gates_test.rs`, `tests/proof_*` |
| `src/wire/` | Kimi Wire JSON-RPC protocol types and client adapter. | `src/wire/README.md` | `tests/wire_protocol_test.rs`, `scripts/kimi-wire-smoke.sh` |
| `src/kimi_native/` | Kimi-native assets, role packs, manifests, sync/doctor support. | `src/kimi_native/README.md` | `tests/kimi_native_test.rs`, `tests/role_pack_test.rs` |
| `src/vis/` | HUD, TUI, event stream, web dashboard scaffold. | `src/vis/mod.rs` | `tests/hud_test.rs` |
| `src/skills/` | OMK skill parser/discovery/injection. | `src/skills/mod.rs` | `tests/skill_test.rs` |
| `src/mcp/` | MCP server scaffold and tools. | `src/mcp/mod.rs` | CLI smoke until deeper MCP tests exist |
| `tests/fixtures/` | Shared mocks and fixture runners. | `tests/fixture_runner.rs` | Used by integration tests |
| `.kimi/` | Project-level Kimi assets for agents/skills/hooks. | `.kimi/skills/omk-navigation/SKILL.md` | `omk kimi doctor` |
| `SPEC.md`, `ROADMAP.md`, `TODO.md` | Product direction, staged delivery path, and `omk goal` backlog. | `SPEC.md` | docs diff review |
| `docs/COMPETITIVE_POSITIONING.md` | Must-have positioning, competitor map, and inspiration/benchmark wording rules. | `docs/COMPETITIVE_POSITIONING.md` | docs diff review |

## Navigation By Task

| If the task is about... | Look Here First | Then Check |
| --- | --- | --- |
| A CLI flag, command, or help output | `src/cli/app.rs`, `src/cli/<command>.rs` | `tests/cli_smoke.rs`, command-specific tests |
| Team worker lifecycle | `src/cli/team.rs`, `src/cli/team/run_support.rs`, `src/runtime/worker.rs`, `src/runtime/state.rs` | `tests/team_lifecycle_test.rs` |
| Team proof/failure artifacts | `src/cli/team/proof.rs`, `src/runtime/proof.rs`, `src/runtime/events.rs` | `cargo test finalize_team_run_proof`, `tests/proof_*` |
| `team run` scheduling | `src/runtime/scheduler/`, `src/runtime/events.rs`, `src/runtime/watchdog.rs` | `tests/ultrawork_test.rs`, `tests/gates_test.rs` |
| Kimi Wire integration | `src/wire/`, `src/runtime/wire_worker.rs` | `tests/wire_protocol_test.rs`, official Wire docs |
| Kimi assets and sync | `src/kimi_native/`, `.kimi/` | `tests/kimi_native_test.rs`, Kimi docs |
| Proof/readiness output | `src/runtime/proof.rs`, `src/cli/proof_cmd.rs` | `tests/proof_cmd_test.rs`, `tests/proof_golden_test.rs` |
| `omk goal` controller scaffold and bounded execution work | `src/runtime/goal/`, `src/cli/goal.rs`, `SPEC.md`, `ROADMAP.md`, `TODO.md` | `tests/goal_cmd_test.rs`, future goal proof tests |
| HUD or event timeline | `src/vis/`, `src/runtime/events.rs` | `tests/hud_test.rs` |
| Cost estimates | `src/cost/`, `src/cli/cost_cmd.rs` | cost tests when added |
| Skills or marketplace | `src/skills/`, `src/marketplace/` | `tests/skill_test.rs`, `tests/marketplace_test.rs` |

## Large Files

Large files are known hotspots, not automatic refactor targets:

- `src/cli/team.rs`: current team command surface for `run`, state inspection, health, shutdown, cleanup, import/export, rename, and roles. Proof artifact writing lives in `src/cli/team/proof.rs`; Wire run support lives in `src/cli/team/run_support.rs`.
- `src/wire/protocol.rs`: Wire JSON-RPC types and parsing contract.
- `src/runtime/autopilot.rs`: autonomous run state machine.
- `src/runtime/events.rs`: event envelope and timeline records.
- `src/runtime/watchdog.rs`: stuck-worker/deadline detection.

When changing one of these, read the area README and modify the smallest coherent region. Avoid opportunistic file splitting unless the user asked for it.

## Agent Workflow

Use this default flow for Kimi K2.6 or any other coding agent:

1. `explore`: map files and symbols for the specific task.
2. `plan`: only when the implementation path has tradeoffs or touches multiple areas.
3. `coder`: edit the smallest file set that satisfies the task.
4. Run focused verification before broad verification.
5. Record known gaps honestly.

For broad changes, create a short task list grouped by area. Do not ask one agent to hold the entire repository in context when a scoped lookup would do.

## Verification Ladder

Pick the smallest proof that matches the change:

```bash
cargo fmt --check
cargo test <specific_test_name>
cargo test --test <integration_test>
cargo clippy --all-targets --all-features -- -D warnings
make check
```

For Wire work, also run:

```bash
scripts/kimi-wire-smoke.sh
```

The Wire smoke script requires a local authenticated `kimi` binary. If it cannot run in the current environment, say so and run the static Rust tests instead.
