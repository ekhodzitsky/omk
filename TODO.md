# OMK TODO

This backlog follows the current product decision: **Kimi-only first**. Provider-neutral workers are deferred until OMK is clearly the best power layer for Kimi CLI.

Use this file as an execution map. It is intentionally detailed and split into parallel lanes so multiple agents or contributors can work without stepping on each other.

## Status Labels

- Current: implemented in the CLI today.
- MVP: usable, but still needs hardening and real-world validation.
- Scaffold: command/module exists, but deeper integration is incomplete.
- Next: planned for the Kimi-only killer demo.
- Later: deferred until the Kimi-only loop is excellent.

## Current Snapshot

- `cargo check` passes on the current worktree.
- `cargo fmt --check` is green.
- `cargo clippy --all-targets --all-features -- -D warnings` is green.
- `cargo test --all-features` is green.
- The worktree contains active in-progress changes for cost tracking, notifications, ultrawork, Kimi-native assets, and CLI expansion.
- `src/kimi_native/manifest.rs` already exists and records OMK-owned assets; it still needs checksums, backup integration, CLI rollback, and stronger doctor support.
- GitHub scan on 2026-05-08 found active Kimi orchestration prior art: `dmae97/oh-my-kimi`, `whatevertogo/oh-my-kimicli`, `mikehenken/kimable`, `geoyws/atmux`, `wang-h/oh-my-kimi-python`, and usage tooling such as `mm7894215/TokenTracker`.
- Official Kimi Code docs are an upstream contract to track before Kimi integration work: <https://www.kimi.com/code/docs>. The Wire Protocol page is first-class for OMK runtime work: <https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html>. See [docs/KIMI_UPSTREAM.md](docs/KIMI_UPSTREAM.md) for the tracked URLs and last checked note. As of 2026-05-08, `kimi info` on Kimi CLI 1.41.0 reports Wire protocol `1.9`.

## North Star

Target demo:

```bash
omk kimi sync
omk team run "fix all failing tests and produce a proof"
omk hud
omk proof show latest
```

`omk kimi sync` is Current Scaffold. `omk team run`, `omk run show`, and `omk proof show` are current paths with remaining polish work.

The demo is successful when a new user can see Kimi workers progressing in parallel, watch a stuck worker recover or fail cleanly, and inspect a final proof with changed files, gates run, failures, retries, known gaps, and final readiness.

## Execution Rules

- Keep README honest: current commands must not be presented as future commands, and future commands must not be presented as current commands.
- Prefer small PRs by lane. Do not mix formatting cleanup, runtime scheduler changes, and docs polish in one PR unless explicitly shipping a release.
- Each lane owns its listed files. If a task needs files from another lane, coordinate before editing.
- Every implementation task must include tests or a written reason why it cannot.
- No new dependencies without a clear entry in this TODO and a short rationale in the commit message.
- Kimi-only first: provider-neutral workers are Later unless they unblock the Kimi-only runtime.
- Wire first: new Kimi process-control work should use the official Kimi Code CLI Wire Protocol before prompt scraping, stdout parsing, or ad hoc bridge protocols.
- Before changing Kimi assets, hooks, agents, MCP config, process launch, worker control, replay, or event capture, re-check <https://www.kimi.com/code/docs> and record any relevant upstream behavior change in `SPEC.md` or this TODO.

## Kimi Implementation Protocol

Kimi should treat this file as an execution contract, not a brainstorming document.

### How Kimi Should Pick Work

1. Start with the highest-priority unblocked lane.
2. Prefer `Next 3` tasks before deeper backlog tasks.
3. Pick one task or one starter batch at a time.
4. Do not mix lanes unless the task explicitly says it touches both.
5. Do not start L10 provider-neutral work before the North Star demo works.
6. When blocked, write down the blocker and switch to another unblocked task in the same lane.

### Task Claim Format

When starting work, Kimi should write or report:

```text
Claim: L2-001
Goal: Define the typed event envelope.
Files expected: src/runtime/events.rs, src/runtime/mod.rs, tests/events_test.rs
Will not touch: src/cli/team.rs, src/vis/*
Verification target: cargo test events
```

### Completion Report Format

When finishing work, Kimi should report:

```text
Completed: L2-001
Changed files:
- src/runtime/events.rs
- src/runtime/mod.rs
- tests/events_test.rs
Verification:
- cargo test events
Evidence:
- Event envelope roundtrip test passes.
Known gaps:
- Event writer not implemented yet; covered by L2-002.
Next suggested task:
- L2-002
```

### Definition Of Ready For Kimi

A task is ready for Kimi when:

- [ ] It has one clear task ID.
- [ ] It has an expected file ownership scope.
- [ ] It has a verification target or an explicit reason verification is not possible yet.
- [ ] It does not depend on an unfinished task unless that dependency is listed.
- [ ] It can be completed without broad product decisions.

### Kimi Safety Rules

- Do not delete user files or state directories unless the task is explicitly about cleanup and has tests or dry-run behavior.
- Do not edit provider-neutral code for L10 while working on Kimi-only critical path tasks.
- Do not add dependencies for checksum, event logging, CLI output, or tests unless the standard library or existing dependencies are clearly insufficient.
- Do not change documented command names without updating `README.md`, `SPEC.md`, `ROADMAP.md`, and this TODO in the same change.
- Do not mark a task complete if tests were not run; mark it "implemented, not verified" with the reason.
- Do not add a new Kimi execution path that depends on natural-language result blocks when `kimi --wire` can provide structured events or requests.

## Starter Work Batches For Kimi

These batches are intentionally small. They are good units for one Kimi session or one PR.

### Batch KIMI-A - Stabilize Formatting

Purpose: make the repository easier to change.

Tasks:

- [x] L0-001 Run `cargo fmt` and make `cargo fmt --check` green.
- [x] L0-007 Confirm `cargo check --all-targets` agrees with default `cargo check`.
- [ ] L0-014 Verify `Makefile` targets match documented development commands.

Expected files:

- Rust files touched by formatter.
- `Makefile` only if target docs are wrong.

Verification:

- `cargo fmt --check`
- `cargo check`
- `cargo check --all-targets`

### Batch KIMI-B - Manifest Checksums

Purpose: make Kimi asset ownership auditable.

Tasks:

- [x] L1-001 Add manifest checksums for files written by `omk kimi sync`.
- [x] L1-004 Decide checksum algorithm and record it in the manifest schema.
- [x] L1-005 Store checksum for every managed file after write.
- [x] L1-013 Add tests for manifest save/load, drift detection, and path normalization.

Expected files:

- `src/kimi_native/manifest.rs`
- `src/kimi_native/sync.rs`
- `src/kimi_native/installer.rs`
- `tests/kimi_native_test.rs`

Verification:

- `cargo test kimi_native`

### Batch KIMI-WIRE - Adopt Official Wire Protocol

Purpose: make `kimi --wire` the default structured worker contract before expanding `omk team run`.

Tasks:

- [x] LW-001 Re-read the official Kimi Code docs root and Wire Protocol page before starting implementation.
- [x] LW-002 Record the observed Kimi CLI version and Wire protocol version in a local fixture or test note.
- [x] LW-003 Add a Rust Wire adapter skeleton for launching `kimi --wire`.
- [x] LW-004 Define JSON-RPC 2.0 request, response, notification, and error envelopes.
- [x] LW-005 Implement `initialize` handshake and fallback for method-not-found no-handshake mode.
- [x] LW-006 Implement a minimal `prompt` request roundtrip with fake-process tests.
- [x] LW-007 Map Wire `event` messages into OMK event log records.
- [x] LW-008 Map Wire `request` messages into explicit OMK approval/question/tool-handling events.
- [x] LW-009 Add cancellation support through Wire `cancel`.
- [ ] LW-010 Decide how Wire `replay` maps to `omk run show` and proof replay.

Expected files:

- `src/runtime/kimi_wire.rs` or `src/runtime/kimi_wire/*`
- `src/runtime/mod.rs`
- `tests/kimi_wire_test.rs`
- fixtures under `tests/fixtures/kimi-wire/`
- `SPEC.md` and this TODO when upstream docs change.

Verification:

- Fake Wire process tests for initialize, prompt, event, request, cancel, and replay fixtures.
- No real Kimi dependency in unit tests.
- Manual real-Kimi smoke test only after the adapter is deterministic.

### Batch KIMI-C - Rollback Dry Run

Purpose: make rollback safe before destructive behavior exists.

Tasks:

- [x] L1-003 Add `omk kimi rollback` CLI using the existing ownership manifest.
- [x] L1-017 Implement rollback dry-run.
- [x] L1-019 Implement rollback report with removed files, restored files, skipped files, and errors.
- [x] L1-021 Ensure rollback handles missing manifest cleanly.

Expected files:

- `src/cli/kimi_native_cmd.rs`
- `src/kimi_native/manifest.rs`
- `src/kimi_native/rollback.rs` if rollback logic does not fit cleanly in `manifest.rs`
- `tests/kimi_native_test.rs`

Verification:

- `cargo test kimi_native`
- Manual: `omk kimi rollback --dry-run` in a temp project.

### Batch KIMI-D - Event Envelope

Purpose: create the evidence layer without touching team runtime yet.

Tasks:

- [x] L2-001 Define the typed event envelope.
- [x] L2-004 Define `RunId`, `EventId`, `WorkerId`, `TaskId`, and `GateId` newtypes.
- [x] L2-005 Define event timestamp format and clock source.
- [x] L2-027 Add event schema version to every event.
- [x] L2-032 Add tests for append/read roundtrip, partial trailing lines, malformed payloads, and filter by kind.

Expected files:

- `src/runtime/events.rs`
- `src/runtime/mod.rs`
- `tests/events_test.rs`

Verification:

- `cargo test events`

### Batch KIMI-E - Event Writer And Reader

Purpose: make events durable and readable.

Tasks:

- [x] L2-002 Add append-only JSONL writer with atomic-ish flush behavior.
- [x] L2-003 Add event reader that tolerates partial/corrupt trailing lines.
- [ ] L2-025 Decide run directory layout for event logs.
- [x] L2-026 Add public event-log naming convention (`events.jsonl` is current; `event-log.jsonl` is a read fallback alias).
- [ ] L2-033 Add tests for partial trailing line.
- [ ] L2-035 Add tests for malformed event payloads.

Expected files:

- `src/runtime/events.rs`
- `tests/events_test.rs`

Verification:

- `cargo test events`

### Batch KIMI-F - Instrument Current Team Spawn

Purpose: add observability before replacing orchestration.

Dependencies:

- L2 event writer exists.

Tasks:

- [x] L3-001 Add append-only event logging to current `omk team spawn`.
- [x] L3-004 Emit `run_started` when `team spawn` creates state.
- [x] L3-005 Emit `worker_started` for each tmux worker pane.
- [x] L3-010 Emit `run_failed` when spawn setup fails after state dir creation.
- [x] L3-012 Add tests using mock worker output.

Expected files:

- `src/cli/team.rs`
- `src/runtime/bridge.rs`
- `src/runtime/events.rs`
- `tests/team_lifecycle_test.rs`

Verification:

- `cargo test team`

### Batch KIMI-G - Proof Schema Only

Purpose: define proof output before wiring gates.

Dependencies:

- L2 event types are stable enough.

Tasks:

- [x] L4-001 Define the typed proof schema.
- [x] L4-004 Define proof status: ready, not_ready, failed.
- [x] L4-011 Define proof JSON path and naming.
- [x] L4-012 Add proof schema golden tests.

Expected files:

- new `src/runtime/proof.rs`
- `src/runtime/mod.rs`
- `tests/proof_test.rs`

Verification:

- `cargo test proof`

### Batch KIMI-H - Mock Kimi Fixture

Purpose: unblock CI demo work without real Kimi.

Tasks:

- [x] L6-001 Create mock Kimi executable fixture.
- [x] L6-004 Mock Kimi supports success mode.
- [x] L6-005 Mock Kimi supports failure mode.
- [x] L6-006 Mock Kimi supports stall mode.
- [x] L6-009 Mock Kimi can run under CI without network.
- [x] L6-010 Mock Kimi can be selected via config/env without touching real Kimi.

Expected files:

- `tests/fixtures/mock-kimi` or equivalent
- `tests/*`
- `examples/killer-demo/*` once the fixture becomes demo-facing

Verification:

- Fixture test that invokes mock Kimi in all modes.

### Batch KIMI-I - README/Tutorial Current Commands

Purpose: keep docs honest while code moves.

Tasks:

- [ ] L8-001 Add Current/Next/Later explanation to docs index or tutorial.
- [ ] L8-002 Add "current commands vs target commands" warning to tutorial.
- [ ] L8-014 Update `docs/TUTORIAL.md` to match Current commands.
- [ ] L8-015 Add tutorial for `omk kimi sync`.
- [ ] L8-016 Add tutorial for `omk team spawn`.

Expected files:

- `docs/TUTORIAL.md`
- `README.md`
- `docs/TROUBLESHOOTING.md` if Kimi/tmux troubleshooting changes

Verification:

- Manual doc review against `omk --help`.

### Batch KIMI-J - First Role Pack Pass

Purpose: make Kimi Pro Mode feel useful immediately.

Dependencies:

- L1 sync format must be stable enough.

Tasks:

- [x] L7-001 Define curated role-pack format.
- [x] L7-012 Architect role.
- [x] L7-013 Executor role.
- [x] L7-014 Verifier role.
- [ ] L7-022 Add prompt anti-slop review pass for every role.
- [ ] L7-024 Ensure roles do not contradict AGENTS.md hierarchy.

Expected files:

- `src/kimi_native/agent_spec.rs`
- `skills/*` if role packs use skills
- tests for generated role specs

Verification:

- `cargo test kimi_native`

## Parallel Work Map

| Lane | Name | Can Start | Blocks | Primary Ownership |
| --- | --- | --- | --- | --- |
| L0 | Stabilize Current v0 | Now | Everything release-facing | repo-wide formatting/tests |
| L1 | Kimi Pro Mode | After L0 fmt pass or in parallel carefully | Demo setup, role packs | `src/kimi_native/*`, `src/cli/kimi_native_cmd.rs`, `tests/kimi_native_test.rs` |
| LW | Kimi Wire Protocol Adapter | Now, after docs re-check | `team run`, structured worker events, replay | `src/runtime/kimi_wire*`, `tests/kimi_wire_test.rs`, Wire fixtures |
| L2 | Event Log Core | After L0 or parallel with isolated files | Proof, HUD timeline, scheduler evidence | `src/runtime/events.rs`, `src/runtime/mod.rs`, event tests |
| L3 | Team Runtime Scheduler | After L2 schema draft | `omk team run`, watchdog, ownership | `src/cli/team.rs`, `src/runtime/state.rs`, new scheduler files |
| L4 | Proof And Gates | After L2 event schema | release-grade completion | `src/runtime/gates.rs`, new proof modules, proof tests |
| L5 | HUD And Observability | After L2 event reader | live demo polish | `src/vis/*`, `src/cli/hud.rs` |
| L6 | Demo And QA Fixtures | After L2 schema draft | launch demo | `tests/*`, `examples/*`, `scripts/*` |
| L7 | Kimi Role Packs | After L1 asset format stabilizes | useful install experience | `src/kimi_native/agent_spec.rs`, `skills/*`, docs |
| L8 | Docs And DX | Always | adoption | `README.md`, `SPEC.md`, `ROADMAP.md`, `docs/*` |
| L9 | Competitive Research | Always | product decisions | docs only |
| L10 | Provider-Neutral Later | After North Star demo | broader market | ask/providers/runtime later |

## Critical Path

1. L0 makes the current tree green.
2. LW implements the official Kimi Wire adapter so new runtime work starts from structured Kimi protocol messages.
3. L2 defines event records and append-only event writing.
4. L3 instruments current `omk team spawn` with events before adding `team run`, then moves `team run` worker control onto Wire.
5. L4 generates `omk proof` from recorded events.
6. L5 makes HUD read the same event/state source.
7. L6 builds the reproducible demo fixture.
8. L1 finishes `sync/doctor/rollback` so the demo starts with safe Kimi assets.

## Demo Acceptance Criteria

- [x] Demo fixture creates three worker outcomes: one success, one failed verification, one stalled worker.
- [ ] HUD shows worker state, heartbeat age, current task, retry count, and verification status.
- [x] Watchdog records a stalled-worker event and either recovers the worker or marks the task failed with evidence.
- [x] Verification gates run after synthesis.
- [x] `omk proof show latest` reports changed files, gates run, failures, retries, known gaps, and final readiness.
- [x] Demo can run against mock Kimi in CI.
- [ ] Demo can run against real Kimi manually.
- [x] Demo script exits non-zero when proof status is `failed`.
- [x] README can link to the demo without pretending Next commands are Current.

---

## L0 - Stabilize Current v0

Goal: make the existing repository trustworthy before adding more surface area.

Primary files: repo-wide, but avoid behavior edits outside failing files.

Can run in parallel with: L8 docs, L9 research. Coordinate with L1/L2/L3 before touching shared runtime files.

### L0 Next 3

- [x] L0-001 Run `cargo fmt` and make `cargo fmt --check` green.
- [x] L0-002 Make `cargo clippy --all-targets --all-features -- -D warnings` green.
- [x] L0-003 Run the full test suite with isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME`.

### L0 Build And Test Cleanup

- [x] L0-004 Record the exact verification commands in `README.md` or `CONTRIBUTING.md`.
- [ ] L0-005 Move network-dependent smoke coverage such as `update --check` behind an ignored/integration test or mock the release lookup.
- [x] L0-006 Ensure `config set` creates the config directory before atomic writes.
- [ ] L0-007 Confirm `cargo check --all-targets` agrees with default `cargo check`.
- [ ] L0-008 Confirm `cargo test --all-targets` does not depend on the user's real home directory.
- [ ] L0-009 Add test helpers for isolated XDG/HOME setup if duplicated across tests.
- [ ] L0-010 Make failing tests print actionable paths and command hints.
- [x] L0-011 Add a CI-friendly smoke test that does not require real Kimi or tmux.
- [x] L0-012 Mark real Kimi/tmux E2E tests as ignored or feature-gated. (No such tests exist in the current suite; all integration tests use mocks or `--help` smoke.)
- [ ] L0-013 Audit warnings suppressed with `allow(...)` and remove unnecessary ones.
- [ ] L0-014 Verify `Makefile` targets match documented development commands.
- [ ] L0-015 Verify `rust-toolchain.toml` matches README Rust version badge.

### L0 Current Surface Honesty

- [x] L0-016 Keep README feature status aligned with actual CLI commands.
- [x] L0-017 Ensure `omk --help` descriptions do not overclaim readiness.
- [x] L0-018 Ensure `omk team --help` names `spawn` as the tmux compatibility path and `run` as the scheduler-backed path.
- [x] L0-019 Ensure `omk kimi --help` mentions Current Kimi commands only.
- [ ] L0-020 Add a command snapshot test, or document why clap output is too unstable for snapshots.
- [ ] L0-021 Update docs when a CLI command is renamed or promoted from Next to Current.

### L0 Definition Of Done

- [x] L0-DOD-001 Formatting, clippy, and tests are green.
- [ ] L0-DOD-002 README documents only current commands as current.
- [ ] L0-DOD-003 `SPEC.md`, `TODO.md`, and `ROADMAP.md` use the same Current/Next/Later vocabulary.
- [ ] L0-DOD-004 Known verification gaps are documented.

---

## L1 - Kimi Pro Mode

Goal: one command safely turns a normal Kimi CLI setup into an OMK-powered setup.

Primary files: `src/kimi_native/*`, `src/cli/kimi_native_cmd.rs`, `tests/kimi_native_test.rs`.

Can run in parallel with: L2 event core, L5 HUD, L8 docs. Coordinate with L7 role packs.

### L1 Next 3

- [x] L1-001 Add manifest checksums for files written by `omk kimi sync`.
- [x] L1-002 Add backups for overwritten Kimi project/user assets.
- [x] L1-003 Add `omk kimi rollback` CLI using the existing ownership manifest and new backups.

### L1 Manifest And Ownership

- [x] L1-004 Decide checksum algorithm and record it in the manifest schema.
- [x] L1-005 Store checksum for every managed file after write.
- [ ] L1-006 Store asset source/version for generated agents, hooks, skills, and config snippets.
- [ ] L1-007 Distinguish created file, overwritten file, unchanged file, and skipped user-owned file.
- [ ] L1-008 Record directories separately from files and remove directories only when empty.
- [x] L1-009 Add manifest schema version migration path.
- [x] L1-010 Add manifest validation to detect corrupt or unsupported manifest versions.
- [x] L1-011 Add drift detection for changed, missing, extra, and user-modified managed files.
- [ ] L1-012 Make drift output human-readable and machine-readable.
- [x] L1-013 Add tests for manifest save/load, drift detection, and path normalization.
- [x] L1-014 Ensure manifest paths are relative and cannot escape project/user roots.

### L1 Backup And Rollback

- [x] L1-015 Create backup files before overwriting any existing non-identical Kimi asset.
- [x] L1-016 Store backup metadata in the manifest or a companion backup index.
- [x] L1-017 Implement rollback dry-run.
- [x] L1-018 Implement rollback apply.
- [x] L1-019 Implement rollback report with removed files, restored files, skipped files, and errors.
- [x] L1-020 Ensure rollback never deletes unrelated user files.
- [x] L1-021 Ensure rollback handles missing manifest cleanly.
- [x] L1-022 Ensure rollback handles partial/corrupt backups cleanly.
- [x] L1-023 Add tests for clean install rollback.
- [x] L1-024 Add tests for overwrite rollback.
- [x] L1-025 Add tests for partial failure rollback.
- [x] L1-026 Add tests for user-modified managed file rollback.

### L1 Sync And Doctor

- [ ] L1-027 Make `omk kimi sync` reconcile project `.kimi/` assets without duplicating stale copies.
- [x] L1-028 Make user-level sync explicit in output so users know project vs user writes.
- [x] L1-029 Add `--dry-run` to `omk kimi sync`.
- [ ] L1-030 Decide and document whether `--project-only` and `--user-only` are required; add them only if sync output shows project/user writes are confusing.
- [x] L1-031 Make `omk kimi doctor` validate Kimi CLI presence and version.
- [x] L1-032 Make `omk kimi doctor` validate agent files.
- [x] L1-033 Make `omk kimi doctor` validate hook scripts and executable bits.
- [ ] L1-034 Make `omk kimi doctor` validate skills paths.
- [ ] L1-035 Make `omk kimi doctor` validate MCP config snippets or references.
- [x] L1-036 Make `omk kimi doctor` validate manifest drift.
- [x] L1-037 Make `omk kimi doctor` print exact repair commands.
- [x] L1-038 Add JSON output mode for doctor when CI or tests need machine-readable diagnostics.

### L1 Kimi Config Integration

- [ ] L1-039 Decide whether OMK writes full Kimi config, snippets, or examples only.
- [ ] L1-040 Document which Kimi config files OMK may modify.
- [ ] L1-041 Add safe merge behavior for hook config if writing config is allowed.
- [ ] L1-042 Preserve user comments when possible or avoid editing comment-heavy configs.
- [ ] L1-043 Add conflict messages when Kimi config already contains incompatible hooks.
- [ ] L1-044 Add tests for config merge or snippet generation.

### L1 Definition Of Done

- [ ] L1-DOD-001 `sync`, `doctor`, and `rollback` can explain every file OMK owns.
- [x] L1-DOD-002 `doctor` can tell the user exactly how to repair stale or missing assets.
- [x] L1-DOD-003 Rollback is tested without touching unrelated user files.
- [x] L1-DOD-004 Kimi asset install is safe to run repeatedly.

---

## LW - Kimi Wire Protocol Adapter

Goal: use the official Kimi Code CLI Wire Protocol as the default structured contract for new Kimi worker control.

Primary files: `src/runtime/kimi_wire.rs` or `src/runtime/kimi_wire/*`, `src/runtime/mod.rs`, `tests/kimi_wire_test.rs`, fixtures under `tests/fixtures/kimi-wire/`.

Can run in parallel with: L1 Kimi Pro Mode and L2 Event Log Core. Blocks target `omk team run` worker control, replay, and clean HUD/proof integration.

Upstream docs to check before each implementation pass:

- Kimi Code docs root: <https://www.kimi.com/code/docs>
- Wire Protocol: <https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html>

As of 2026-05-08, `kimi info` on Kimi CLI 1.41.0 reports `kimi --wire` protocol version `1.9` over JSON-RPC 2.0, one JSON message per line.

### LW Next 3

- [x] LW-001 Re-read official Kimi docs and record observed Wire protocol version.
- [x] LW-002 Add Wire adapter module skeleton and fake-process fixture harness.
- [x] LW-003 Define JSON-RPC 2.0 envelope types and ID handling.

### LW Docs Watch

- [x] LW-004 Add a short `docs/KIMI_UPSTREAM.md` or equivalent section that records tracked Kimi docs URLs and last checked date.
- [x] LW-005 Record Kimi CLI version and Wire protocol version in test fixtures or snapshot notes.
- [x] LW-006 Add a release checklist item to re-check Kimi docs before publishing OMK releases.
- [ ] LW-007 Track upstream changes to agents, hooks, skills, MCP, config files, and Wire Protocol behavior.

### LW Process And Handshake

- [x] LW-008 Launch `kimi --wire` with piped stdin/stdout and stderr capture.
- [x] LW-009 Implement JSONL framing: one JSON-RPC message per line.
- [x] LW-010 Implement `initialize` with client name/version and expected protocol negotiation.
- [x] LW-011 Implement fallback when `initialize` returns method-not-found and upstream no-handshake mode is required.
- [x] LW-012 Record Kimi binary path, CLI version if available, and Wire protocol version in run metadata.
- [x] LW-013 Add process shutdown and child cleanup behavior.
- [x] LW-014 Add tests for startup failure, malformed output, and EOF handling.

### LW Requests, Events, And Control

- [x] LW-015 Implement `prompt` request and response handling.
- [x] LW-016 Implement streaming `event` notification ingestion.
- [x] LW-017 Map Wire `event` notifications into OMK `events.jsonl` records.
- [x] LW-018 Implement Wire `request` handling for approvals, questions, tool calls, or future upstream request kinds.
- [x] LW-019 Add explicit unknown-method and unknown-event handling.
- [x] LW-020 Implement `cancel` support.
- [x] LW-021 Implement `steer` support for active turns if it fits OMK safety rules.
- [ ] LW-022 Implement `set_plan_mode` only after the UX/safety implications are documented.
- [x] LW-023 Add timeout behavior for stuck Wire turns.

### LW Replay And Proof Integration

- [x] LW-024 Implement `replay` support against `wire.jsonl` or upstream-compatible transcript fixtures.
- [x] LW-025 Decide whether OMK stores raw Wire logs, normalized OMK events, or both. Decision: store both; raw Wire logs are redacted before durability, and normalized OMK events power `run show`, HUD, and proof.
- [x] LW-026 Redact sensitive Wire payload fields before writing durable logs.
- [x] LW-027 Make `omk run show` able to reference Wire-derived events.
- [x] LW-028 Make `omk proof` include Wire-derived task/output/request evidence.

### LW Team Runtime Integration

- [x] LW-029 Make target `omk team run` start workers through the Wire adapter by default.
- [ ] LW-030 Keep tmux visibility for users while Wire owns structured worker control.
- [ ] LW-031 Keep current text/tmux bridge as compatibility fallback for `team spawn`.
- [ ] LW-032 Add config or feature flag for disabling Wire only when debugging compatibility.
- [ ] LW-033 Add mock Wire worker fixture for scheduler tests.

### LW Definition Of Done

- [x] LW-DOD-001 Fake Wire tests cover initialize, prompt, event, request, cancel, replay, malformed output, and EOF.
- [x] LW-DOD-002 `team run` design no longer depends on prompt-shaped result blocks for normal worker output.
- [x] LW-DOD-003 Run metadata records Kimi binary path, Kimi CLI version when available, and Wire protocol version.
- [x] LW-DOD-004 Docs clearly state Wire-first behavior and the tmux/text fallback boundary.

---

## L2 - Event Log Core

Goal: create the append-only runtime evidence layer shared by team, proof, HUD, and demo.

Primary files: new `src/runtime/events.rs`, `src/runtime/mod.rs`, tests under `tests/`.

Can run in parallel with: L1 Kimi Pro Mode, L8 docs. Blocks large parts of L3/L4/L5/L6.

### L2 Next 3

- [x] L2-001 Define the typed event envelope.
- [x] L2-002 Add append-only JSONL writer with atomic-ish flush behavior.
- [x] L2-003 Add event reader that tolerates partial/corrupt trailing lines.

### L2 Event Model

- [x] L2-004 Define `RunId`, `EventId`, `WorkerId`, `TaskId`, and `GateId` newtypes.
- [x] L2-005 Define event timestamp format and clock source.
- [x] L2-006 Define `run_started` and `run_completed`.
- [x] L2-007 Define `run_failed`.
- [x] L2-008 Define `worker_started`.
- [x] L2-009 Define `worker_heartbeat`.
- [x] L2-010 Define `worker_stalled`.
- [x] L2-011 Define `worker_recovered`.
- [x] L2-012 Define `task_claimed`.
- [x] L2-013 Define `task_started`.
- [x] L2-014 Define `task_output`.
- [x] L2-015 Define `task_completed`.
- [x] L2-016 Define `task_failed`.
- [x] L2-017 Define `file_changed`.
- [x] L2-018 Define `command_started`.
- [x] L2-019 Define `command_finished`.
- [x] L2-020 Define `gate_passed`.
- [x] L2-021 Define `gate_failed`.
- [x] L2-022 Define `retry_scheduled`.
- [x] L2-023 Define `proof_written`.
- [x] L2-024 Define `manual_interrupt`.

### L2 Storage And Compatibility

- [ ] L2-025 Decide run directory layout for event logs.
- [x] L2-026 Add public event-log naming convention (`events.jsonl` is current; `event-log.jsonl` is a read fallback alias).
- [x] L2-027 Add event schema version to every event.
- [ ] L2-028 Decide event log rotation policy; default to no rotation until a measured fixture produces oversized logs.
- [x] L2-029 Add event reader filters by event kind, worker, task, gate, and time.
- [x] L2-030 Add event summary builder.
- [ ] L2-031 Add event redaction hook for sensitive command output.
- [x] L2-032 Add tests for append/read roundtrip.
- [x] L2-033 Add tests for partial trailing line.
- [ ] L2-034 Add tests for unknown future event kinds.
- [x] L2-035 Add tests for malformed event payloads.

### L2 Definition Of Done

- [x] L2-DOD-001 Events can be written and read without invoking Kimi.
- [x] L2-DOD-002 Event logs are deterministic enough for golden tests.
- [x] L2-DOD-003 Partial/corrupt event logs produce useful diagnostics, not panics.

---

## L3 - Kimi Team Runtime Scheduler

Goal: make multi-Kimi execution reliable enough to leave running unattended.

Primary files: `src/cli/team.rs`, `src/runtime/state.rs`, `src/runtime/worker.rs`, `src/runtime/bridge.rs`, new scheduler/watchdog modules.

Can run in parallel with: L1, L5 after L2 schema. Coordinate heavily with L2 and L4.

### L3 Next 3

- [x] L3-001 Add append-only event logging to current `omk team spawn`.
- [ ] L3-002 Introduce task IDs, terminal states, and retry records in team state.
- [ ] L3-003 Add watchdog events for dead panes, stalled heartbeats, and stuck Kimi execution.

### L3 Current Spawn Instrumentation

- [x] L3-004 Emit `run_started` when `team spawn` creates state.
- [x] L3-005 Emit `worker_started` for each tmux worker pane.
- [x] L3-006 Emit `worker_heartbeat` from bridge loop.
- [x] L3-007 Emit `task_started` when worker consumes inbox task.
- [x] L3-008 Emit `task_completed` when worker writes success to outbox.
- [x] L3-009 Emit `task_failed` when worker writes failure to outbox.
- [x] L3-010 Emit `run_failed` when spawn setup fails after state dir creation.
- [x] L3-011 Emit `manual_interrupt` or shutdown event from `team shutdown`.
- [x] L3-012 Add tests using mock worker output.

### L3 Scheduler Types

- [ ] L3-013 Define task status enum: pending, claimed, running, succeeded, failed, cancelled, stale.
- [ ] L3-014 Define worker status enum: starting, ready, busy, stalled, dead, stopped.
- [ ] L3-015 Define claim record with task id, worker id, lease deadline, attempt number.
- [ ] L3-016 Define retry policy with max attempts, backoff, and terminal failure.
- [ ] L3-017 Define ownership scope model: files, directories, globs, unknown.
- [ ] L3-018 Define conflict detection result.
- [ ] L3-019 Add serialization tests for scheduler state.

### L3 Atomic Claims And Leases

- [ ] L3-020 Implement atomic task claim write.
- [ ] L3-021 Implement lease renewal.
- [ ] L3-022 Implement stale lease detection.
- [ ] L3-023 Implement stale lease recovery.
- [ ] L3-024 Implement retry scheduling.
- [ ] L3-025 Prevent two workers from claiming the same task.
- [ ] L3-026 Add tests for concurrent claim attempts.
- [ ] L3-027 Add tests for stale lease takeover.

### L3 Watchdog

- [x] L3-028 Define heartbeat timeout config.
- [x] L3-029 Detect missing heartbeat.
- [x] L3-030 Detect unchanged heartbeat beyond threshold.
- [x] L3-031 Detect missing tmux pane/session.
- [ ] L3-032 Detect stuck `kimi --print` or non-TTY execution through timeout.
- [x] L3-033 Record `worker_stalled`.
- [ ] L3-034 Attempt recovery when safe.
- [ ] L3-035 Record `worker_recovered` or terminal failure.
- [x] L3-036 Add tests for dead pane simulation.
- [x] L3-037 Add tests for stale heartbeat simulation.

### L3 `omk team run`

- [x] L3-038 Add CLI shape for `omk team run`.
- [x] L3-039 Add `--workers <N>`.
- [x] L3-040 Add `--role <ROLE>`.
- [x] L3-041 Add `--dir <PATH>`.
- [x] L3-042 Add `--name <RUN_NAME>`.
- [x] L3-043 Add `--gate <NAME>` or equivalent gate selection.
- [ ] L3-044 Add `--yolo` only if consistent with safety model.
- [x] L3-045 Create run manifest.
- [x] L3-046 Start lead Kimi (TeamRunner orchestrates in OMK CLI).
- [x] L3-047 Start worker Kimi processes via WireWorkerAdapter.
- [x] L3-048 Decompose task into explicit scheduled work units (lead-agent decomposition via Wire + static fallback).
- [x] L3-049 Add final synthesis as scheduled task (post-run synthesis agent via Wire).
- [x] L3-050 Keep `omk team spawn` as compatibility command until `run` is mature.

### L3 Definition Of Done

- [x] L3-DOD-001 A mock team run can complete with multiple workers and deterministic state.
- [x] L3-DOD-002 A stalled worker is detected and recorded.
- [x] L3-DOD-003 A failed worker produces evidence, not silent success.
- [ ] L3-DOD-004 Parallel workers cannot claim conflicting file ownership scopes without a warning or block.

---

## L4 - Proof And Verification Gates

Goal: completion is based on evidence, not agent confidence.

Primary files: `src/runtime/gates.rs`, new proof modules, new CLI modules for `run`/`proof`, tests.

Can run in parallel with: L3 after L2 event schema. Blocks launch demo.

### L4 Next 3

- [x] L4-001 Define the typed proof schema.
- [x] L4-002 Add `omk run show <id|latest>` for event timeline inspection.
- [x] L4-003 Add `omk proof show <id|latest>` for readiness reports from event logs.

### L4 Proof Model

- [x] L4-004 Define proof status: ready, not_ready, failed.
- [x] L4-005 Define changed file summary.
- [x] L4-006 Define gate result summary.
- [x] L4-007 Define failure summary.
- [x] L4-008 Define retry summary.
- [x] L4-009 Define known gap summary.
- [x] L4-010 Define final readiness text.
- [x] L4-011 Define proof JSON path and naming.
- [x] L4-012 Add proof schema golden tests.
- [x] L4-013 Add proof markdown/text renderer after JSON proof is stable and README/demo needs human-readable output.

### L4 Run Show

- [x] L4-014 Add `omk run list`.
- [x] L4-015 Add `omk run show <id>`.
- [x] L4-016 Add `omk run show latest`.
- [x] L4-017 Add filtering by worker/task/kind.
- [x] L4-018 Add concise timeline output.
- [x] L4-019 Add JSON output option.
- [x] L4-020 Add tests for latest run resolution.

### L4 Verification Gates

- [x] L4-021 Define `VerificationGate` config model.
- [x] L4-022 Support fmt gate.
- [x] L4-023 Support lint/clippy gate.
- [x] L4-024 Support typecheck/check gate.
- [x] L4-025 Support test gate.
- [ ] L4-026 Support security gate placeholder.
- [ ] L4-027 Support docs gate.
- [ ] L4-028 Support custom command gates.
- [x] L4-029 Capture command stdout/stderr summaries.
- [x] L4-030 Store full command output path when needed.
- [x] L4-031 Add timeout per gate.
- [ ] L4-032 Add allow-fail gate mode for informational checks.
- [ ] L4-033 Add retry/fix loop hook driven by failed gate evidence.
- [x] L4-034 Add tests for passing gates.
- [x] L4-035 Add tests for failing gates.
- [ ] L4-036 Add tests for skipped gates.

### L4 Completion Contract

- [x] L4-037 Make no team/autopilot/ralph run report success without a proof artifact or explicit failure artifact.
- [x] L4-038 Add final "done contract" fields: changed files, gates run, evidence, known gaps.
- [x] L4-039 Add failure artifact for interrupted runs.
- [x] L4-040 Add proof generation from recorded event logs without rerunning Kimi.
- [x] L4-041 Add proof generation for partial/corrupt logs with warnings.

### L4 Definition Of Done

- [x] L4-DOD-001 A proof contains changed files, gates run, command evidence, failures, retries, known gaps, and final readiness.
- [x] L4-DOD-002 A proof can be generated from a recorded event log without rerunning Kimi.
- [x] L4-DOD-003 Failed and partial runs produce useful proof/failure artifacts.

---

## L5 - HUD And Observability

Goal: make parallel Kimi work understandable while it is happening and after it finishes.

Primary files: `src/vis/*`, `src/cli/hud.rs`, event readers.

Can run in parallel with: L1 and L7. Needs L2 for final event-backed version.

### L5 Next 3

- [x] L5-001 Make HUD read current team state plus event logs when available.
- [ ] L5-002 Show worker heartbeat age and stalled/dead status.
- [ ] L5-003 Show current task, retry count, and verification status.

### L5 Tmux Statusline

- [ ] L5-004 Show active team count.
- [ ] L5-005 Show running/stalled/dead worker counts.
- [x] L5-006 Show latest failed gate.
- [x] L5-007 Show latest proof status.
- [x] L5-008 Keep output compact enough for tmux status bar.

### L5 TUI

- [ ] L5-009 Add run selector.
- [ ] L5-010 Add worker list.
- [ ] L5-011 Add task list.
- [ ] L5-012 Add event timeline.
- [ ] L5-013 Add gate/proof panel.
- [ ] L5-014 Add stalled worker highlighting.
- [ ] L5-015 Add keyboard help.
- [ ] L5-016 Add empty-state screen when no runs exist.

### L5 Web Dashboard

- [ ] L5-017 Add event-backed run timeline endpoint.
- [ ] L5-018 Add proof endpoint.
- [ ] L5-019 Add worker status endpoint.
- [ ] L5-020 Add basic web page for live team state.
- [ ] L5-021 Start with simple auto-refresh; add SSE/WebSocket only if polling makes the HUD visibly stale or expensive.
- [ ] L5-022 Add no-JS fallback if simple.
- [ ] L5-023 Add tests for server routes.

### L5 Cost And Metrics

- [ ] L5-024 Show estimated cost by run.
- [ ] L5-025 Show estimated cost by worker.
- [ ] L5-026 Show token/cost unknown state honestly.
- [ ] L5-027 Record cost events in event log when available.
- [ ] L5-028 Add cost report by mode and role.

### L5 Definition Of Done

- [ ] L5-DOD-001 HUD can explain what every worker is doing or why it stopped.
- [x] L5-DOD-002 HUD distinguishes current state from final proof.
- [x] L5-DOD-003 HUD works with mock run fixture.

---

## L6 - Demo And QA Fixtures

Goal: make the North Star demo reproducible and testable.

Primary files: `tests/*`, `examples/*`, `scripts/*`, docs.

Can run in parallel with: L1/L2/L5 after interfaces stabilize.

### L6 Next 3

- [x] L6-001 Create mock Kimi executable fixture.
- [x] L6-002 Create scripted team fixture with success, failure, and stall.
- [x] L6-003 Create expected proof golden output for the fixture.

### L6 Mock Kimi

- [x] L6-004 Mock Kimi supports success mode.
- [x] L6-005 Mock Kimi supports failure mode.
- [x] L6-006 Mock Kimi supports stall mode.
- [x] L6-007 Mock Kimi supports slow streaming mode.
- [x] L6-008 Mock Kimi supports malformed output mode.
- [x] L6-009 Mock Kimi can run under CI without network.
- [x] L6-010 Mock Kimi can be selected via config/env without touching real Kimi.

### L6 Demo Script

- [x] L6-011 Add `examples/killer-demo/README.md`.
- [x] L6-012 Add demo setup script.
- [x] L6-013 Add demo run script.
- [x] L6-014 Add demo cleanup script.
- [x] L6-015 Add demo expected outputs.
- [x] L6-016 Ensure demo exits non-zero when proof status is failed.
- [x] L6-017 Ensure demo can run in temporary directory.
- [x] L6-018 Ensure demo does not mutate user Kimi config unless explicitly requested.

### L6 QA Matrix

- [x] L6-019 Test no Kimi installed.
- [ ] L6-020 Test Kimi installed but unauthenticated.
- [ ] L6-021 Test no tmux installed.
- [ ] L6-022 Test tmux session already exists.
- [ ] L6-023 Test stale state directory.
- [ ] L6-024 Test read-only project directory.
- [ ] L6-025 Test worker dies mid-task.
- [x] L6-026 Test gate command times out.
- [x] L6-027 Test proof generation after interrupted run.
- [ ] L6-028 Test rollback after partial sync failure.

### L6 Definition Of Done

- [x] L6-DOD-001 Demo can run against mock Kimi in CI.
- [ ] L6-DOD-002 Demo can run against real Kimi manually.
- [x] L6-DOD-003 Demo proof output is stable enough for docs/screenshots.

---

## L7 - Kimi Role Packs

Goal: make OMK useful immediately on real projects.

Primary files: `src/kimi_native/agent_spec.rs`, `skills/*`, role docs.

Can run in parallel with: L1 after role-pack format is agreed.

### L7 Next 3

- [ ] L7-001 Define curated role-pack format.
- [ ] L7-002 Ship first-party Kimi role packs for Rust, frontend, security, docs, QA, and release.
- [ ] L7-003 Add `omk kimi doctor` validation for repo-local role overrides.

### L7 Role Pack Format

- [ ] L7-004 Define role id naming rules.
- [ ] L7-005 Define role description field.
- [ ] L7-006 Define system prompt field.
- [ ] L7-007 Define tool/permission hints if supported by Kimi.
- [ ] L7-008 Define recommended gates per role.
- [ ] L7-009 Define ownership scope hints per role.
- [ ] L7-010 Define maturity label per role pack.
- [ ] L7-011 Add parser/validation tests.

### L7 First-Party Roles

- [ ] L7-012 Architect role.
- [ ] L7-013 Executor role.
- [ ] L7-014 Verifier role.
- [ ] L7-015 Reviewer role.
- [ ] L7-016 Security reviewer role.
- [ ] L7-017 Rust maintainer role.
- [ ] L7-018 Frontend fixer role.
- [ ] L7-019 Docs writer role.
- [ ] L7-020 Test repair role.
- [ ] L7-021 Release captain role.

### L7 Role Quality

- [ ] L7-022 Add prompt anti-slop review pass for every role.
- [ ] L7-023 Ensure each role prompt stays under a documented length budget.
- [ ] L7-024 Ensure roles do not contradict AGENTS.md hierarchy.
- [ ] L7-025 Add examples for role selection.
- [ ] L7-026 Compare against `dmae97/oh-my-kimi` role coverage.
- [ ] L7-027 Compare against `whatevertogo/oh-my-kimicli` skills/hooks.

### L7 Definition Of Done

- [ ] L7-DOD-001 Role packs are Kimi-native and can be synced, validated, and rolled back.
- [ ] L7-DOD-002 Examples demonstrate realistic work on a repo.
- [ ] L7-DOD-003 Bad role-pack config is caught by `omk kimi doctor`.

---

## L8 - Docs And Developer Experience

Goal: make the project understandable, honest, and easy to try.

Primary files: `README.md`, `SPEC.md`, `ROADMAP.md`, `TODO.md`, `docs/*`, `examples/*`.

Can run in parallel with: all lanes, but must track actual CLI status.

### L8 Next 3

- [x] L8-001 Add Current/Next/Later explanation to docs index or tutorial.
- [x] L8-002 Add "current commands vs target commands" warning to tutorial.
- [ ] L8-003 Add quick command reference generated from `--help` or manually verified.

### L8 README

- [ ] L8-004 Keep first screen focused on Kimi-only wedge.
- [ ] L8-005 Keep competitor list out of README except one link to SPEC/competitive scan.
- [ ] L8-006 Keep quickstart runnable on current CLI.
- [ ] L8-007 Keep North Star demo clearly marked as target.
- [ ] L8-008 Add short animated/demo artifact later when available.

### L8 SPEC

- [ ] L8-009 Keep Current v0 and Target v1 separate.
- [ ] L8-010 Update capability table after every command promotion.
- [ ] L8-011 Add state diagrams when scheduler design stabilizes.
- [ ] L8-012 Add event schema examples as implementation lands.
- [ ] L8-013 Add proof examples as implementation lands.

### L8 Tutorials And Examples

- [x] L8-014 Update `docs/TUTORIAL.md` to match Current commands.
- [x] L8-015 Add tutorial for `omk kimi sync`.
- [x] L8-016 Add tutorial for `omk team spawn`.
- [x] L8-017 Add tutorial coverage for `omk team run` now that it is implemented.
- [ ] L8-018 Add troubleshooting page for Kimi auth.
- [ ] L8-019 Add troubleshooting page for tmux.
- [ ] L8-020 Add troubleshooting page for stale state and rollback.

### L8 Definition Of Done

- [ ] L8-DOD-001 A new user can tell what works today within 60 seconds.
- [ ] L8-DOD-002 Future commands are never presented as current.
- [ ] L8-DOD-003 Docs link to proof/demo once available.

---

## L9 - Competitive Research

Goal: keep the product sharper than current Kimi orchestration prior art.

Primary files: `SPEC.md`, optional future `docs/COMPETITIVE.md`.

Can run in parallel with: all lanes.

### L9 Research Tasks

- [ ] L9-001 Maintain the prior-art table in `SPEC.md`.
- [ ] L9-002 Compare `dmae97/oh-my-kimi` install flow against OMK.
- [ ] L9-003 Compare `dmae97/oh-my-kimi` team runtime against OMK.
- [ ] L9-004 Compare `dmae97/oh-my-kimi` DAG/ensemble planning against OMK scheduler plans.
- [ ] L9-005 Compare `dmae97/oh-my-kimi` hooks and MCP integration against OMK Kimi Pro Mode.
- [ ] L9-006 Compare `dmae97/oh-my-kimi` graph memory against OMK project memory plans.
- [ ] L9-007 Compare `whatevertogo/oh-my-kimicli` hook install/status/rollback behavior against OMK.
- [ ] L9-008 Validate `mikehenken/kimable` warning that `kimi --print` / `--quiet` can hang in non-TTY environments.
- [ ] L9-009 Borrow useful architecture ideas from `geoyws/atmux`: pull-based task claims, lanes, watchdogs, digests, audit/drift detection.
- [ ] L9-010 Benchmark token/cost UX against `TokenTracker`.
- [ ] L9-011 Re-scan GitHub before public launch.
- [ ] L9-012 Keep direct competitor details out of README unless strategically useful.

### L9 Definition Of Done

- [ ] L9-DOD-001 Competitive findings produce concrete tasks, not vague inspiration.
- [ ] L9-DOD-002 README positioning remains confident, not defensive.
- [ ] L9-DOD-003 SPEC contains enough prior art for future product decisions.

---

## L10 - Later Provider-Neutral Control Plane

Goal: expand after the Kimi-only runtime is excellent.

Status: Later.

Do not pull these into the critical path before the North Star demo works.

### L10 Later Tasks

- [ ] L10-001 Keep Kimi as the default and best-supported execution provider.
- [ ] L10-002 Keep Codex, Gemini, Claude, and OpenCode as optional advisors/workers only after the Kimi-only loop is excellent.
- [ ] L10-003 Add provider capability metadata: context size, preferred roles, cost tier, supports JSONL, supports images, supports background work.
- [ ] L10-004 Add provider routing rules that are explicit and inspectable.
- [ ] L10-005 Add cross-provider review workflows where one provider builds and another reviews.
- [ ] L10-006 Add provider-specific timeout/deadlock behavior.
- [ ] L10-007 Add provider-specific cost accounting.
- [ ] L10-008 Add provider-neutral role compatibility matrix.
- [ ] L10-009 Add provider-neutral proof evidence normalization.

---

## Better Than OMC Criteria

OMK is not better than OMC when it merely has the same command names. It is better when:

- [ ] A run can be replayed from structured events.
- [ ] Completion is determined by verification evidence, not agent confidence.
- [ ] Parallel workers cannot silently overwrite each other's ownership scope.
- [ ] Kimi-native assets and external tmux-backed Kimi workers can be used in the same workflow.
- [ ] The user can inspect cost, failures, retries, changed files, and final proof from one command.
- [ ] The runtime degrades gracefully when Kimi, tmux, network, or optional providers are unavailable.

## Launch Readiness Checklist

- [ ] README first screen is crisp and Kimi-only.
- [ ] Quickstart works on a clean machine with Kimi and tmux installed.
- [ ] `omk kimi sync` is safe and reversible.
- [ ] `omk team run` or its demo equivalent handles success, failure, and stall.
- [ ] `omk hud` makes parallel work legible.
- [ ] `omk proof show latest` explains final readiness.
- [ ] Mock Kimi CI demo is green.
- [ ] Real Kimi manual demo instructions are documented.
- [ ] Known limitations are honest and visible.
- [ ] The release has a demo GIF/video or terminal recording.
