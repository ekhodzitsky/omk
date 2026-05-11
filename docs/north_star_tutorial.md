# North Star Tutorial

This tutorial walks through the **North Star Demo** — the target oh-my-kimi workflow that shows Kimi agents fixing code, producing a proof, and reporting status.

> **Maturity note:** `omk kimi sync`, `omk team run`, `omk hud`, `omk run show`, and `omk proof show` are in the CLI today. The remaining work is proof/HUD polish and hardening, not command invention. For the tutorial covering only today's CLI surface, see [TUTORIAL.md](TUTORIAL.md).

## North Star Commands (Target Workflow)

```bash
omk kimi sync
omk team run 2:coder "fix all failing tests and produce a proof"
omk hud --once
omk proof show latest
```

The demo is successful when you can see Kimi workers progressing in parallel, watch a stuck worker recover or fail cleanly, and inspect a final proof or failure artifact with changed files, gates run, failures, retries, known gaps, and final readiness.

---

## Prerequisites

- **Rust toolchain** (1.78+) for building `omk`
- **Kimi CLI** installed and authenticated (or use `MOCK_KIMI=1` for a fully offline demo)
- **tmux** available on your system (`brew install tmux` or `apt install tmux`)
- **Python 3** (only needed for the wire-compatible mock when running with `MOCK_KIMI=1`)

---

## Quick Start

### 1. Build and install `omk`

```bash
git clone https://github.com/ekhodzitsky/oh-my-kimi
cd oh-my-kimi
cargo build --release
# Optional: copy to a location on your PATH
cp target/release/omk ~/.local/bin/
```

### 2. Run the demo script

```bash
./scripts/north_star_demo.sh
```

For a fully mocked run (no real Kimi API calls):

```bash
MOCK_KIMI=1 ./scripts/north_star_demo.sh
```

Other environment variables you can set:

| Variable | Effect |
|----------|--------|
| `MOCK_KIMI=1` | Use the built-in wire-compatible mock instead of real Kimi |
| `NORTH_STAR_DRY_RUN=1` | Run `omk kimi sync --dry-run` instead of a real sync |
| `NORTH_STAR_NO_CLEANUP=1` | Keep the temp project and team state after the demo |

---

## What the Script Does

### Step 1 — Setup

- Detects the `omk` binary (installed → `target/release/omk` → `cargo run --`)
- Detects whether to use real Kimi or a mock
- Creates a **temporary Rust project** with an intentional failing test:
  ```rust
  #[test]
  fn test_add() {
      assert_eq!(add(2, 2), 5); // wrong on purpose
  }
  ```

### Step 2 — `omk kimi sync`

Synchronizes OMK Kimi-native assets (agents, hooks, skills) into the current project. With `NORTH_STAR_DRY_RUN=1` this shows what would change without touching files.

### Step 3 — `omk team run`

Launches a team of 2 coder workers with the task *"fix the failing test and make cargo test pass"*.

> **Status:** `omk team run` is available in the current CLI. `omk team spawn` remains the compatibility path for the older tmux bridge.

What happens under the hood in the target design:

1. **Lead decomposition** — a lead agent breaks the task into parallel subtasks (e.g. "fix the add function", "run cargo test").
2. **Worker dispatch** — each subtask is claimed by a wire worker and written to the worker's `inbox.jsonl`.
3. **Execution** — each worker spawns a `kimi --wire` process, sends the task, and collects results.
4. **Polling & synthesis** — the scheduler polls worker `outbox.jsonl` files, marks tasks complete, and runs a synthesis agent to produce a final summary.
5. **Verification gates** — `cargo fmt`, `cargo check`, `cargo clippy`, and `cargo test` are run automatically. With `MOCK_KIMI=1`, the script first proves the fixture fails, then applies a deterministic fixture repair so the offline proof path can finish green.

### Step 4 — `omk hud`

Prints a one-shot JSON snapshot of the team:

```json
{
  "run_id": "north-star-demo",
  "team_name": "north-star-demo",
  "task_summary": {
    "total": 3,
    "completed": 3,
    "running": 0,
    "pending": 0,
    "failed": 0
  },
  "workers": [...]
}
```

### Step 5 — `omk proof show latest`

Reads the run's cached `proof.json` when present, or regenerates a proof report from `events.jsonl`:

> **Status:** `omk proof show` exists in the CLI today. The hardening work is in richer gate reporting, replay, and demo polish.

- **Status** — `Ready`, `NotReady`, or `Failed`
- **Changed files** — list of files modified during the run
- **Gates** — verification results (fmt, check, clippy, test)
- **Failures** — any worker or gate failures
- **Retries** — tasks that were retried after stale-lease recovery
- **Known gaps** — explicitly acknowledged incomplete work

`omk proof show` supports all three demo formats:

```bash
omk proof show latest --format text
omk proof show latest --format md
omk proof show latest --format json
```

The demo script validates the JSON verdict and exits non-zero when the final proof `status` is `failed`.

With a real Kimi, the proof should show `Ready` plus the files Kimi actually changed. With `MOCK_KIMI=1`, the script keeps OMK state isolated, repairs the tiny fixture deterministically, and expects `Ready` with passing gates.

### Step 6 — Cleanup

Removes the temporary project and all team state (unless `NORTH_STAR_NO_CLEANUP=1` is set).

---

## Using with Real Kimi

Unset `MOCK_KIMI` and ensure the `kimi` CLI is authenticated:

```bash
kimi --version        # should print version
kimi info             # should show wire protocol 1.9 or the currently supported protocol
kimi auth status      # should show you are logged in
```

Then run the demo without the mock:

```bash
./scripts/north_star_demo.sh
```

> ⚠️ **Cost warning**: running with real Kimi will consume API tokens. The demo creates 2 workers + 1 lead + 1 synthesis agent, each making at least one LLM call.

If you are validating a new Kimi CLI release, first run `kimi info` and compare it with [KIMI_UPSTREAM.md](KIMI_UPSTREAM.md). Extension fields in `initialize.result`, such as `hooks`, can evolve while the protocol remains compatible, so OMK should parse them as structured JSON evidence rather than a closed schema.

---

## Troubleshooting

### "No teams found" when running `omk hud` or `omk proof`

You need to run `omk team run` first so that team state exists on disk. The demo script does this automatically.

### "Dead workers" in HUD output

- Check that `kimi --version` works.
- If using `MOCK_KIMI=1`, verify Python 3 is available (`python3 --version`).
- Check `~/.local/state/omk/team/<name>/workers/*/heartbeat.json` for worker status.

### "Empty proof" or "No events found for run"

- The run may have failed before writing events. Check the run output for errors.
- With `MOCK_KIMI=1`, the wire mock may have crashed. Look at `events.jsonl` in the team state directory for malformed lines.
- If gates failed, the proof status will be `Failed` rather than `Ready` — this is still a valid proof, it just means the work did not pass verification.

### `omk team run` hangs

The scheduler waits for workers to complete. With real Kimi, workers can take minutes. With the mock, it should finish in under 10 seconds. If it hangs:

- Check `~/.local/state/omk/team/<name>/workers/*/inbox.jsonl` — tasks should be written there.
- Check `~/.local/state/omk/team/<name>/workers/*/outbox.jsonl` — results should appear there.
- Check `~/.local/state/omk/team/<name>/events.jsonl` — events track the run lifecycle.

### Real Kimi fails during `initialize`

- Rebuild OMK after Wire protocol changes: `cargo build --bin omk`.
- Check the local protocol report: `kimi info`.
- Run a minimal handshake outside the demo and inspect whether `initialize.result` has new extension fields.
- Record upstream drift in [KIMI_UPSTREAM.md](KIMI_UPSTREAM.md) before changing runtime parsing.

### `cargo test` in the temp project does not fail

The script creates a fixture with `assert_eq!(add(2, 2), 5)`. If your Rust version or test runner formats output differently, the grep check may miss it. The script warns and continues — the fixture itself is still correct.

---

## File Reference

| File | Purpose |
|------|---------|
| `scripts/north_star_demo.sh` | The demo script (this tutorial's companion) |
| `~/.local/state/omk/team/<name>/events.jsonl` | Event log driving HUD and proof |
| `~/.local/state/omk/team/<name>/event-log.jsonl` | Compatibility read alias when the canonical event log is absent |
| `~/.local/state/omk/team/<name>/workers/*/inbox.jsonl` | Tasks dispatched to each worker |
| `~/.local/state/omk/team/<name>/workers/*/outbox.jsonl` | Results returned by each worker |
| `~/.local/state/omk/team/<name>/proof.json` | Cached proof report |
| `~/.local/state/omk/team/<name>/failure.json` | Failure summary emitted for failed, not-ready, or interrupted runs |

---

## Next Steps

- Read the full [TUTORIAL.md](TUTORIAL.md) for the current CLI surface: team spawn, autopilot, skill management, and `omk kimi sync`.
- Read [ARCHITECTURE.md](ARCHITECTURE.md) to understand how the scheduler, wire protocol, and proof system fit together.
- Read [SPEC.md](../SPEC.md) for the product roadmap and design decisions.
