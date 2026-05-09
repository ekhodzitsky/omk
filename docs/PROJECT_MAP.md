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
omk team run "fix all failing tests and produce a proof"
omk hud
omk proof show latest
```

Some of that surface is still scaffold/roadmap. Check `README.md`, `SPEC.md`, and `ROADMAP.md` before promising a command is fully implemented.

## Official Kimi Contract

Use official Kimi Code docs as the source of truth before changing Kimi integration:

- Docs root: https://www.kimi.com/code/docs
- Wire Protocol: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html
- Skills: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills.html
- Subagents: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/sub-agents.html

Default rule: new Kimi process-control work is Wire-first. Prefer `kimi --wire` events and requests over prompt scraping.
The current observed Wire protocol version is `1.9`, and legacy/no-handshake fallback remains valid when upstream does not support `initialize`.
Upstream tracking notes live in [KIMI_UPSTREAM.md](KIMI_UPSTREAM.md).

## Repository Areas

| Area | Purpose | Start With | Common Tests |
| --- | --- | --- | --- |
| `src/main.rs` | Top-level CLI wiring and command dispatch. | `README.md`, `src/cli/README.md` | `tests/cli_smoke.rs` |
| `src/cli/` | Clap command handlers and user-facing command behavior. | `src/cli/README.md` | `tests/*_test.rs` matching the command |
| `src/runtime/` | State, process control, scheduler, events, proof, watchdogs. | `src/runtime/README.md` | `tests/team_lifecycle_test.rs`, `tests/gates_test.rs`, `tests/proof_*` |
| `src/wire/` | Kimi Wire JSON-RPC protocol types and client adapter. | `src/wire/README.md` | `tests/wire_protocol_test.rs`, `scripts/kimi-wire-smoke.sh` |
| `src/kimi_native/` | Kimi-native assets, role packs, manifests, sync/doctor support. | `src/kimi_native/README.md` | `tests/kimi_native_test.rs`, `tests/role_pack_test.rs` |
| `src/vis/` | HUD, TUI, event stream, web dashboard scaffold. | `src/vis/mod.rs` | `tests/hud_test.rs` |
| `src/skills/` | OMK skill parser/discovery/injection. | `src/skills/mod.rs` | `tests/skill_test.rs` |
| `src/mcp/` | MCP server scaffold and tools. | `src/mcp/mod.rs` | CLI smoke until deeper MCP tests exist |
| `tests/fixtures/` | Shared mocks and fixture runners. | `tests/fixture_runner.rs` | Used by integration tests |
| `.kimi/` | Project-level Kimi assets for agents/skills/hooks. | `.kimi/skills/omk-navigation/SKILL.md` | `omk kimi doctor` |

## Navigation By Task

| If the task is about... | Look Here First | Then Check |
| --- | --- | --- |
| A CLI flag, command, or help output | `src/main.rs`, `src/cli/<command>.rs` | `tests/cli_smoke.rs`, command-specific tests |
| Team worker lifecycle | `src/cli/team.rs`, `src/runtime/worker.rs`, `src/runtime/state.rs` | `tests/team_lifecycle_test.rs` |
| `team run` scheduling | `src/runtime/scheduler/`, `src/runtime/events.rs`, `src/runtime/watchdog.rs` | `tests/ultrawork_test.rs`, `tests/gates_test.rs` |
| Kimi Wire integration | `src/wire/`, `src/runtime/wire_worker.rs` | `tests/wire_protocol_test.rs`, official Wire docs |
| Kimi assets and sync | `src/kimi_native/`, `.kimi/` | `tests/kimi_native_test.rs`, Kimi docs |
| Proof/readiness output | `src/runtime/proof.rs`, `src/cli/proof_cmd.rs` | `tests/proof_cmd_test.rs`, `tests/proof_golden_test.rs` |
| HUD or event timeline | `src/vis/`, `src/runtime/events.rs` | `tests/hud_test.rs` |
| Cost estimates | `src/cost/`, `src/cli/cost_cmd.rs` | cost tests when added |
| Skills or marketplace | `src/skills/`, `src/marketplace/` | `tests/skill_test.rs`, `tests/marketplace_test.rs` |

## Large Files

Large files are known hotspots, not automatic refactor targets:

- `src/cli/team.rs`: current team command surface, including `spawn` and `run`, plus tmux team operations.
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
