# OMK Specification

## Overview

OMK (oh-my-kimi) is a Rust orchestration layer for Kimi CLI. It runs outside Kimi CLI, starts real `kimi` processes, coordinates them through Kimi Wire Protocol, tmux, and state files, and records enough state to make multi-agent work observable and recoverable.

The current product lane is **Kimi-only first**. Provider-neutral workers can return later, but the first public push should make OMK the best power layer for Kimi CLI.

## Status Vocabulary

| Label | Meaning |
| --- | --- |
| Current | Implemented in the CLI today. |
| MVP | Usable, but still needs hardening and real-world validation. |
| Scaffold | Command/module exists, but the full product behavior is incomplete. |
| Next | Planned for the Kimi-only killer demo. |
| Later | Deferred until the Kimi-only loop is excellent. |

## Product Direction

OMK is inspired by oh-my-claudecode, but it is not a line-for-line port. The goal is to build a Kimi-native runtime that can use Kimi CLI's own primitives where they are stronger:

- Kimi Code CLI Wire Protocol for structured bidirectional control,
- custom agent files for specialist roles,
- Kimi-compatible skills,
- Kimi lifecycle hooks,
- Kimi MCP configuration,
- print and stream output for programmatic execution,
- tmux workers for visible long-running Kimi processes.

The product position:

> OMC is an orchestration plugin. OMK should become a Kimi orchestration runtime.

This means LLMs may plan and execute, but the Rust runtime owns durable state, scheduling, retries, verification gates, conflict detection, recovery, and observability.

## Upstream Kimi Contract

OMK must track official Kimi Code documentation as the source of truth for Kimi integration work:

- Docs root: <https://www.kimi.com/code/docs>
- Wire Protocol: <https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html>

The default integration path for new Kimi process-control work is **Wire first**.

As of 2026-05-08, `kimi info` on Kimi CLI 1.41.0 reports Wire protocol `1.9`, and the official Wire Protocol page documents `kimi --wire` as the low-level structured communication mode for external programs. It uses JSON-RPC 2.0 over stdin/stdout, one JSON message per line. Relevant methods and message flows include `initialize`, `prompt`, `replay`, `steer`, `set_plan_mode`, `cancel`, `event`, and `request`.

Implications for OMK:

- New scheduler, HUD, proof, and replay work should use a `kimi --wire` adapter before inventing prompt scraping or result-block parsing.
- The adapter must record the observed Kimi CLI version and negotiated Wire protocol version in run metadata.
- If `initialize` returns method-not-found, OMK should fall back to legacy/no-handshake Wire mode as documented upstream.
- Tmux remains useful for visible sessions and current `team spawn` compatibility, but target `team run` should treat Wire events/requests as the structured worker contract.
- Any task touching Kimi assets, hooks, agents, MCP, process launch, event capture, replay, or worker control must re-check the official docs before implementation.

## Current v0 Surface

This section describes what the CLI exposes today.

| Capability | Status | Notes |
| --- | --- | --- |
| Rust CLI surface | Current | Commands include team, autopilot, ralph, ask, hud, mcp-server, doctor, config, backup, state, cleanup, skill, marketplace, cost, ultrawork, and kimi. |
| `omk kimi sync` | Current Scaffold | Syncs project/user Kimi assets and writes a project manifest. Needs checksums, backups, rollback CLI, stronger tests, and docs polish. |
| `omk kimi doctor` | Current Scaffold | Validates Kimi-native project assets and suggests fixes. Needs version compatibility checks and deeper manifest-aware repair. |
| `omk kimi install` | Current Scaffold | Installs project Kimi assets. Needs clearer relationship with `sync`. |
| `omk team spawn` | Current MVP | Starts lead/worker Kimi processes in tmux, uses JSONL inbox/outbox and heartbeats. |
| `omk team list/status/attach/broadcast/shutdown` | Current MVP | Operates on current team state and tmux sessions. |
| Autopilot | Current MVP | Six-phase state machine with resume/yolo, phase logs, fallback content, cost tracking, and notifications. |
| Ralph | Current MVP | Persistent verify/fix loop with PRD state, iteration limits, resume/yolo, cost tracking, and notifications. |
| Ultrawork | Current MVP | CLI and runtime exist; needs formatting, focused tests, and real Kimi execution validation. |
| Skills | Current MVP | Parser/discovery, bundled skills, user install/list/search/remove, and marketplace registry support. |
| MCP | Current Scaffold | Server command exists; deeper tool coverage and integration tests are still needed. |
| HUD and web dashboard | Current Scaffold | Needs event-log timeline and richer runtime visibility. |
| Cost tracking | Current MVP | Estimated session costs are recorded; provider-accurate accounting remains future work. |
| Notifications | Current MVP | Discord, Slack, Telegram event formatting exists; event coverage and delivery tests are incomplete. |
| `omk team run` | Current MVP | Scheduler-backed Kimi-only entrypoint with claims, leases, and watchdogs. |
| `omk run show` | Current Scaffold | Event timeline inspection for recorded runs. |
| `omk proof show` | Current Scaffold | Readiness report from event logs and verification gates. |
| `omk kimi rollback` | Current Scaffold | Manifest-backed rollback via CLI; clean no-op when no manifest exists. |
| Provider-neutral workers | Later | Deferred until Kimi-only runtime is excellent. |

## Kimi-Only Killer Feature Set

The first public product push should center on one cohesive target workflow:

```bash
omk kimi sync
omk team run "fix all failing tests and produce a proof"
omk hud
omk proof show latest
```

The commands `omk team run` and `omk proof show` exist in the CLI today. The remaining work is proof hardening and richer replay/filtering, not command invention.

The killer features behind the workflow:

1. **Kimi Pro Mode**: `omk kimi sync` installs or reconciles Kimi-compatible agents, skills, hooks, MCP config, backups, and ownership metadata.
2. **Kimi Team Runtime**: `omk team run` starts a Kimi lead and Kimi workers with role assignment, task claims, leases, ownership scopes, retries, and watchdogs.
3. **Live Kimi HUD**: `omk hud` shows workers, tasks, heartbeats, file ownership, retries, tests, cost estimates, and stuck/hung processes.
4. **Proof And Replay**: `omk proof show` reports changed files, verification gates, failures, retries, known gaps, and final readiness from structured evidence rather than agent self-report.
5. **Crash And Deadlock Recovery**: the runtime detects stalled Kimi processes, `kimi --print` / non-TTY hangs, stale leases, dead workers, and partially completed tasks, then recovers or explains the stop condition.
6. **Curated Kimi Role Packs**: role packs are Kimi-native and practical: Rust maintainer, frontend fixer, security reviewer, docs writer, test repair, release captain, and repo-specific workflows.

## Target v1 Architecture

Target v1 has three cooperating lanes.

### Native Kimi Lane

Use Kimi-compatible assets for work that should happen inside one Kimi session or one Kimi project.

1. `omk kimi sync` installs and reconciles:
   - `.kimi/agents/*.yaml`
   - `.kimi/hooks/*.sh`
   - `.kimi/skills/*/SKILL.md`
   - Kimi MCP configuration
   - OMK ownership manifests and backups
2. `omk kimi doctor` validates assets, versions, permissions, missing files, stale files, and fix hints.
3. `omk kimi rollback` exposes manifest-backed rollback and backup restore through the CLI.
4. Kimi hooks write lifecycle events into OMK run state.

### External Runtime Lane

Use Rust and tmux when OMK needs visibility, process control, recovery, or long-running parallel work.

1. `omk team run` creates a durable run directory under the OMK state root.
2. The Rust scheduler owns task state, worker state, claims, leases, retries, and final synthesis status.
3. Worker processes are real `kimi` CLI processes in the primary Kimi-only lane.
4. New worker control uses `kimi --wire` and maps Wire `event` / `request` messages into OMK events.
5. Tmux panes remain available for visibility and current `team spawn` compatibility.
6. Prompt-shaped result extraction is a fallback path, not the target contract.
7. The watchdog detects dead panes, stale heartbeats, stuck Wire turns, stuck non-TTY execution, and stale leases.

### Proof Lane

Use append-only events to make completion explainable.

1. Every mode writes events to `event-log.jsonl`.
2. Verification gates write command evidence and summaries.
3. `omk run show <id|latest>` reads the timeline.
4. `omk proof show <id|latest>` creates a final readiness report.
5. A run is not complete until it has a proof artifact or an explicit failure artifact.

## Current v0 Team Mode

Current team mode is `spawn` for the tmux bridge; `run` is also available for the scheduler-backed path.

### Command

```bash
omk team spawn <N:ROLE> [OPTIONS] <TASK...>
```

Example:

```bash
omk team spawn 3:coder "fix all TypeScript errors"
```

### Current Spawn Flow

1. Parse `N:ROLE`, for example `3:coder`.
2. Generate a team name or accept `--name`.
3. Ensure tmux is installed.
4. Create state directory under the OMK team state root.
5. Write `team-state.json` with initial state.
6. Create tmux session `omk-team-<name>`.
7. In pane 0, spawn lead `kimi -p <lead prompt>`.
8. For each worker:
   - create `workers/worker-<i>/`,
   - write `worker-spec.json`,
   - split the tmux window,
   - spawn a bridge that polls `inbox.jsonl` and launches Kimi work,
   - write `heartbeat.json` and `outbox.jsonl`.
9. Print team summary and attach/status/shutdown instructions.

### Current State Files

```text
team-state.json
workers/<worker>/worker-spec.json
workers/<worker>/inbox.jsonl
workers/<worker>/outbox.jsonl
workers/<worker>/heartbeat.json
```

### Current Limitations

- The lead prompt still owns much of the orchestration.
- There is no central scheduler with leases and stale-lease recovery.
- File ownership and conflict detection are not first-class.
- Completion is not governed by one proof contract.
- Worker output still depends on text/result extraction paths.

## Target v1 Team Run

Target team mode is `run`.

### Command

```bash
omk team run [OPTIONS] <TASK...>
```

Expected options:

- `--workers <N>`
- `--role <ROLE>`
- `--dir <PATH>`
- `--gate <NAME>`
- `--name <RUN_NAME>`
- `--yolo`

### Target Run Flow

1. Create a run manifest with task, roles, gates, worker count, and state paths.
2. Write an append-only `event-log.jsonl`.
3. Start a Kimi lead and Kimi workers.
4. Scheduler assigns tasks through atomic claims and leases.
5. Workers emit structured events.
6. Watchdog recovers dead/stuck workers or records a failure event.
7. Verification gates run after synthesis.
8. `proof.json` records final readiness, known gaps, and evidence.

## Event Schema

Target event records are JSONL lines with a common envelope:

```json
{
  "id": "uuid",
  "run_id": "string",
  "ts": "2026-05-08T12:00:00Z",
  "kind": "task_started",
  "actor": "worker-0",
  "payload": {}
}
```

Required event kinds:

- `run_started`
- `worker_started`
- `worker_heartbeat`
- `task_claimed`
- `task_started`
- `task_output`
- `file_changed`
- `command_started`
- `command_finished`
- `gate_passed`
- `gate_failed`
- `retry_scheduled`
- `worker_stalled`
- `worker_recovered`
- `run_failed`
- `run_completed`
- `proof_written`

## Proof Schema

Target proof shape:

```json
{
  "run_id": "string",
  "status": "ready|not_ready|failed",
  "changed_files": ["path"],
  "gates": [
    {
      "name": "cargo test",
      "status": "passed|failed|skipped",
      "evidence_event_id": "uuid"
    }
  ],
  "failures": [],
  "retries": [],
  "known_gaps": [],
  "summary": "string"
}
```

## Migration Plan

1. Stabilize current v0 docs and code: formatting, clippy, tests, current command docs.
2. Finish Kimi asset safety: manifest checksums, backups, `doctor` drift checks, and rollback CLI.
3. Add event logging to current `team spawn` before replacing orchestration behavior.
4. Add `omk run show` and proof generation from recorded event logs.
5. Add scheduler-owned claims, leases, ownership scopes, and watchdog recovery.
6. Introduce `omk team run` as the polished v1 entrypoint.
7. Keep `omk team spawn` as a compatibility command until `run` is mature.

## Prior Art And Competitive Scan

As of 2026-05-08, the Kimi orchestration space is no longer empty. OMK should treat these projects as validation and pressure to differentiate, not as reasons to stop.

| Project | Signal | Product lesson |
| --- | --- | --- |
| [MoonshotAI/kimi-cli](https://github.com/MoonshotAI/kimi-cli) | Official Kimi CLI with MCP, ACP, shell mode, and agent execution primitives. | Build on Kimi-native capabilities instead of emulating Claude-only workflows. |
| [dmae97/oh-my-kimi](https://github.com/dmae97/oh-my-kimi) | Strongest direct competitor: TypeScript/npm CLI, worktree team runtime, DAG/ensemble planning, MCP skill-hooks, quality gates, graph memory, and provider routing claims. Its maturity notes mark some team/runtime surfaces as alpha or experimental. | Competing only on command names is not enough. OMK needs a clearer reliability/runtime story. |
| [whatevertogo/oh-my-kimicli](https://github.com/whatevertogo/oh-my-kimicli) | Personal Kimi workflow layer with skills, hooks, continuation, review, insights, setup/update/uninstall, and project state. | Asset sync must be disciplined: install, status, backup, rollback, and uninstall should be first-class. |
| [mikehenken/kimable](https://github.com/mikehenken/kimable) | Claude-to-Kimi delegation/orchestration adapter using `kimi --agent-file`; notes that `kimi --print` / `--quiet` can hang in some non-TTY environments. | Structured protocol work must include TTY-safe execution, timeouts, and compatibility fallbacks. |
| [geoyws/atmux](https://github.com/geoyws/atmux) | tmux-native multi-TUI orchestrator for Claude Code, OpenCode, Kimi, and Cursor CLI with pull-based task claims, lanes, watchdogs, and digests. | The team scheduler should use explicit claims, dependencies, lane isolation, watchdogs, and audit trails. |
| [wang-h/oh-my-kimi-python](https://github.com/wang-h/oh-my-kimi-python) | Python-native Kimi orchestrator with tmux teams, HUD, instruction overlays, and MCP-style state. | Rust is OMK's differentiator only if it produces a more reliable runtime, not just another wrapper. |
| [mm7894215/TokenTracker](https://github.com/mm7894215/TokenTracker) | Cross-agent CLI token and usage tracker including Kimi. | Cost and usage visibility should be part of the product, not an afterthought. |

## Test Strategy

- Unit tests: parsers, state serialization, event schema, proof schema, asset manifests, command generation.
- Integration tests: mock Kimi team run, JSONL flow, event logs, proof generation, rollback behavior.
- Fixture tests: one successful worker, one failed worker, one stuck worker, expected proof output.
- E2E tests: real Kimi CLI and tmux, run manually or in gated CI environments.
