# OMK Roadmap

This roadmap captures the current product decision: **Kimi-only first**.

OMK should become the best power layer for Kimi CLI before it expands into a generic control plane for every AI coding agent. The goal is not to copy OMC command names. The goal is to make Kimi teams observable, recoverable, and provable.
Official Kimi docs and local upstream notes are tracked in [docs/KIMI_UPSTREAM.md](docs/KIMI_UPSTREAM.md).

## Status Labels

- Current: implemented in the CLI today.
- Next: planned for the Kimi-only killer demo.
- Later: deferred until the Kimi-only runtime is excellent.

## North Star

Target demo:

```bash
omk kimi sync
omk team run "fix all failing tests and produce a proof"
omk hud
omk proof show latest
```

`omk kimi sync` is Current Scaffold. `omk team run`, `omk run show`, and `omk proof show` exist today, with the remaining work focused on hardening and demo polish.

The demo should show a real or mock Kimi team working in parallel, a live HUD, a stuck or failed worker being handled, verification gates running, and a final proof artifact that explains what happened.

## Demo Acceptance Criteria

The first launch demo must be reproducible.

- It can run against mock Kimi in CI.
- It can run against real Kimi manually.
- It creates three worker outcomes:
  - one successful worker,
  - one worker with failed verification,
  - one stalled worker detected by the watchdog.
- HUD shows worker status, task status, heartbeat age, retry count, and verification status.
- Watchdog records a recovery or terminal failure event for the stalled worker.
- The final proof includes changed files, gates run, failures, retries, known gaps, and readiness.
- The demo script exits non-zero when proof status is `failed`.

## Milestone 0 - Stabilize Current v0

Goal: make the existing repository trustworthy before adding more surface area.

Scope:

- Make `cargo fmt --check` green.
- Make `cargo clippy --all-targets --all-features -- -D warnings` green.
- Run the full test suite with isolated home/config/cache directories.
- Gate or finish in-progress surfaces such as ultrawork, cost tracking, notifications, HUD, and MCP.
- Keep README maturity labels honest: Current, MVP, Scaffold, Next, Later.

Definition of Done:

- Formatting, clippy, and tests are green.
- README documents only current commands as current.
- `SPEC.md`, `TODO.md`, and `ROADMAP.md` use the same vocabulary.

## Milestone 1 - Kimi Pro Mode

Goal: one command safely turns a normal Kimi CLI setup into an OMK-powered setup.

Current:

- `omk team run`
- `omk kimi sync`
- `omk kimi doctor`
- `omk kimi install`
- `omk kimi agents`
- `omk kimi hooks`
- `omk kimi skills`

Next:

- `omk kimi rollback`
- manifest checksums,
- backups before overwrite,
- manifest-aware `doctor` repair hints,
- tests for clean install, overwrite, partial failure, and rollback.

Definition of Done:

- `sync`, `doctor`, and `rollback` can explain every file OMK owns.
- Rollback does not touch unrelated user files.
- Kimi asset install is safe to run repeatedly.

## Milestone 2 - Kimi Team Runtime

Goal: make multi-Kimi execution reliable enough to leave running unattended.

Current:

- `omk team run`
- `omk team spawn`
- `omk team list`
- `omk team status`
- `omk team attach`
- `omk team broadcast`
- `omk team shutdown`

Next:

- runtime-owned task claims,
- leases and stale-lease recovery,
- file ownership scopes,
- watchdog for dead panes, stalled heartbeats, stuck `kimi --print`, non-TTY hangs, and partial task completion,
- live HUD backed by runtime events.

Definition of Done:

- A mock team can run deterministically in tests.
- A stalled worker is detected and recorded.
- A failed worker produces evidence.
- `omk team spawn` remains available or has a documented migration path.

## Milestone 3 - Proof And Replay

Goal: completion is based on evidence, not agent confidence.

Next:

- append-only `event-log.jsonl` for every run,
- `omk run show <id|latest>` for timeline inspection, including Wire-derived event/request details,
- `omk proof show <id|latest>` for final readiness, including Wire evidence and malformed-log warnings,
- verification gates for fmt, lint, typecheck, tests, security, docs, and custom commands,
- recovery evidence for crashes, timeouts, deadlocks, stale leases, and manual interrupts.

Definition of Done:

- A proof can be generated from a recorded event log without rerunning Kimi.
- Failed and partial runs produce useful proof/failure artifacts.
- A run cannot silently claim success without proof or explicit failure.

## Milestone 4 - Kimi Role Packs

Goal: make OMK useful immediately on real projects.

Next:

- curated Kimi-native role packs instead of a broad low-quality marketplace,
- packs for Rust, frontend, backend, security, documentation, QA, release, and migration work,
- repo-local role overrides with `omk kimi doctor` validation,
- examples that show complete workflows, not just isolated commands.

Definition of Done:

- Role packs are Kimi-native and can be synced, validated, and rolled back.
- Bad role-pack config is caught by `omk kimi doctor`.
- Examples demonstrate realistic work on a repo.

## Later - Provider-Neutral Control Plane

Provider-neutral workers remain valuable, but they are not the first wedge.

Before expanding beyond Kimi workers, OMK should prove:

- Kimi sync is safe and reversible.
- Kimi team runs are observable and recoverable.
- `omk proof show` can explain completion better than agent self-report.
- The HUD makes parallel work understandable.
- The product has a demo that makes users want to install it immediately.

After that, Codex, Gemini, Claude, and OpenCode can return as optional advisors or workers.
