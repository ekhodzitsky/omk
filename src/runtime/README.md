# Runtime Area Map

`src/runtime/` is OMK's control plane. It owns durable state, process control, scheduling, retries, verification evidence, and recovery behavior.

## Files

| File | Owns |
| --- | --- |
| `ask.rs` | Provider ask execution helpers. |
| `atomic.rs` | Atomic file writes. |
| `autopilot.rs` | Autopilot state machine and execution loop. |
| `config.rs` | XDG and legacy path resolution. |
| `events.rs` | Event envelope and timeline records. |
| `gates.rs` | Verification gate definitions and execution. |
| `metrics.rs` | Runtime metrics. |
| `migrate.rs` | State schema migration. |
| `proof.rs` | Proof/readiness report data and synthesis. |
| `ralph.rs` | Ralph persistence loop. |
| `retry.rs` | Retry/backoff helpers. |
| `scheduler/` | Task claims, leases, ownership, and runner scaffold. |
| `shell.rs` | Shell escaping and validation. |
| `state.rs` | Team/autopilot/ralph state files. |
| `ultrawork.rs` | Parallel burst execution runtime. |
| `watchdog.rs` | Dead/stuck worker detection. |
| `wire_worker.rs` | Kimi Wire worker adapter used by `team run`. |
| `worker.rs` | JSONL inbox/outbox worker IPC. |

## Edit Rules

- Runtime changes should be observable through state, events, logs, or proof output.
- Prefer append-only event records for behavior that must be audited later.
- Use `atomic.rs` for state/proof/event artifacts that readers may inspect while a run is active.
- Keep `team run` Wire-first; do not add terminal-session orchestration back into the runtime.
- Do not silently change state file shape without migration or a compatibility note.

## Tests

Useful starting points:

```bash
cargo test --test team_lifecycle_test
cargo test --test gates_test
cargo test --test proof_cmd_test
cargo test --test proof_golden_test
cargo test --test ultrawork_test
```

When touching state migration or filesystem layout, include a focused test with temporary HOME/XDG directories.
