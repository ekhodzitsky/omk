# OMK Tutorial

This tutorial covers the current OMK MVP surface: Kimi-native assets,
scheduler-backed `team run`, HUD/run/proof inspection, and cleanup.

## Chat REPL

The default entry point is the chat REPL. Run `omk` with no arguments to start a session, type a request, and press Enter. The classifier routes trivial answers directly, small edits through a fast dispatch path, and large goals into the durable `omk goal` controller. Press `Tab` to expand the engine pane and watch autonomous decisions in real time. Press `Ctrl+C` or `Ctrl+D` to quit.

Resume a previous session with `omk chat --session <id>` or start a fresh one with `omk chat --new`.

## Prerequisites

- Rust 1.78+ if building from source.
- Kimi CLI installed and authenticated for real agent runs.
- Python 3 only when running the offline `MOCK_KIMI=1` demo.

OMK is GitHub-only for now. It is not published to crates.io yet.

## Install

From GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash
```

Or build from source:

```bash
git clone https://github.com/ekhodzitsky/oh-my-kimi.git
cd oh-my-kimi
cargo build --release
./target/release/omk --help
```

## First Setup

```bash
omk setup
omk doctor
```

`omk setup` creates config, data, cache, and state directories. `omk doctor`
checks the local OMK environment and Kimi availability. Real team runs require
Kimi CLI auth:

```bash
kimi --version
kimi auth status
```

## First Goal

The shortest lazy flow is one setup step and one goal command:

```bash
omk setup
omk goal run "Build a tiny local-only Rust CLI with tests and proof evidence" --until-ready
```

During the run, the terminal prints the goal id, status, phase, state path, and
proof path. Inspect the persisted progress and proof afterward:

```bash
omk goal replay latest
omk goal proof latest --format md
```

In the current beta, `--until-ready` records the intent and creates the durable
controller scaffold. If execution, review, or integration evidence is still
missing, the proof remains `not_ready` and lists the exact gap instead of
claiming success.

## Sync Kimi Assets

Preview first:

```bash
omk kimi sync --dry-run
```

Then write the managed Kimi assets:

```bash
omk kimi sync
omk kimi doctor
```

Useful inspection commands:

```bash
omk kimi agents
omk kimi hooks
omk kimi skills
omk kimi rollback --dry-run
```

## Run a Team

The current team workflow is Wire-first:

```bash
omk team run 2:executor "fix failing tests and produce a proof"
```

What OMK records during the run:

- team state under the active OMK state root;
- worker inbox/outbox JSONL files;
- heartbeat files;
- `events.jsonl`;
- verification gate output artifacts;
- `proof.json` for ready runs or `failure.json` for failed/not-ready runs.

Use a clean branch. Agent runs can edit your repository.

## Inspect the Run

```bash
omk team status <team-name>
omk team health <team-name>
omk run list
omk run show latest
omk run show latest --json
omk proof show latest
omk proof show latest --format md
omk hud --once
omk hud --json
```

`run show` answers "what happened?" from the event timeline. `proof show`
answers "is this ready?" from gates, changed files, failures, retries, known
gaps, and Wire evidence.

## Shutdown and Cleanup

```bash
omk team shutdown <team-name>
omk team cleanup --dry-run --older-than 7
omk cleanup --teams --dry-run
```

`team shutdown` marks the state interrupted and writes failure evidence when the
run is not already ready. Cleanup commands should be dry-run first.

## Offline Demo

Run the North Star demo without real Kimi API calls:

```bash
NORTH_STAR_DRY_RUN=1 bash scripts/north_star_demo.sh
```

The dry-run path isolates `HOME`/`XDG_*`, forces a mock Kimi runtime even when
real Kimi is installed, creates a tiny Rust fixture, runs `omk setup`, runs one
`omk goal run --until-ready`, and then inspects replay/proof evidence.

## Greenfield Goal Demo

Use this as the first honest `omk goal` greenfield acceptance example. It is a
small engineering fixture, not a product launch flow. The happy path starts
with one command; the later commands are inspection and explicit recovery
surfaces, not required steering steps.

```bash
tmpdir="$(mktemp -d)"
cd "$tmpdir"
cargo new omk-goal-greenfield-demo
cd omk-goal-greenfield-demo
omk setup
omk goal run \
  "Build a tiny local-only Rust CLI named taskline. It should support add <text> and list commands, store tasks in tasks.txt, include tests for both commands, avoid network access, and add no new dependencies." \
  --until-ready \
  --budget-time 30m \
  --budget-tokens 200000 \
  --max-agents 1
omk goal show latest
omk goal proof latest --format md
omk goal replay latest --format text
```

`goal run` creates the durable acceptance artifact set: `goal.json`, `prd.md`,
`technical-plan.md`, `test-spec.md`, `task-graph.json`, `decisions.jsonl`, and
an initial `proof.json`. When the controller needs explicit recovery, use
`goal verify` for gate evidence, `goal execute` for bounded Wire worker
evidence, `goal review` for controller review/security artifacts, and
`goal open-pr latest --dry-run` for a local PR draft without network mutation.

For the Rust fixture, OMK auto-detects the default required gates:

- `cargo fmt --check`
- `cargo check --all-targets`
- `cargo clippy -- -D warnings`
- `cargo test`

Treat passing gates plus execution/review evidence as **engineering-ready
evidence**: the requested behavior has a PRD, plan, test spec, task graph, gate
outputs, changed-file evidence, and review/security notes. It is
**product-ready** only after a human accepts the diff, commits it, opens or
merges a PR, and handles release-level docs or positioning. Until that
integration acceptance exists, `omk goal proof` is expected to remain
`not_ready` and name the remaining gap.

For the fuller artifact map and troubleshooting notes, see
[north_star_tutorial.md](north_star_tutorial.md).

## Rewrite, Audit, and Refactor Goal Examples

Rewrite goals should name the source behavior that must stay compatible:

```bash
omk goal run "Rewrite this tiny Python CLI in Rust. Preserve stdout, stderr, exit codes, and generated files for the existing hello command." --until-ready --budget-time 1h --max-agents 1
omk goal verify latest
omk goal execute latest
omk goal review latest
omk goal accept latest --summary "compatibility and golden evidence accepted"
```

Refactor goals should name the behavior that must not change:

```bash
omk goal run "Refactor src/runtime/goal/replay.rs for clarity without changing replay JSON output, duplicate collapse, or malformed-line recovery." --until-ready --budget-time 45m --max-agents 1
```

Audit goals should name the evidence artifact and gate that closes the audit:

```bash
omk goal run "Audit goal proof output for secret leakage. Produce a security review artifact and keep all configured tests passing." --until-ready --budget-time 45m --max-agents 1
```

For rewrite/refactor/migration plans, `test-spec.md` lists compatibility and
golden oracle checks plus any detected source command/API/file surfaces. For
greenfield plans, `artifacts/oracles/` contains local acceptance, demo, and
usage-example scaffolds.

## Verification Gates

OMK has built-in gate presets for common stacks and supports project overrides:

```bash
cat .omk/gates.toml
omk team run 1:executor --gate fmt,check,test "finish the change"
```

Required gates block ready proof generation when they fail. Allow-fail and
skipped gates are recorded explicitly so proof output stays honest.

## Power-User Modes

```bash
omk autopilot "build a small REST API"
omk ralph --max-iterations 5 "make tests pass"
omk ultrawork --concurrency 4 "task one" "task two" "task three"
```

These are useful, but the strongest MVP path is still:

```text
omk kimi sync -> omk team run -> omk run/proof/hud -> local verification
```

## Common Issues

### Kimi CLI not found

```bash
kimi --version
which kimi
omk doctor
```

Install and authenticate Kimi CLI using the official upstream instructions, then
retry from the same shell.

### Kimi Wire initialize fails

```bash
kimi info
cargo build --bin omk
```

Compare the local protocol report with `docs/KIMI_UPSTREAM.md`. New Kimi
versions may add extension fields while remaining protocol-compatible.

### Team run hangs

Check the state files for the team:

```bash
omk team health <team-name>
omk run show latest
omk proof show latest
```

Then inspect the active state root if needed:

```bash
ls ~/.local/state/omk/team/<team-name>
ls ~/.omk/state/team/<team-name>
```

OMK prefers `~/.omk/state` when a legacy `~/.omk/` directory exists; otherwise
it uses XDG state under `~/.local/state/omk`.

## Current CLI Surface

```text
omk setup
omk doctor
omk kimi sync/install/doctor/rollback/agents/hooks/skills
omk team run/status/health/shutdown/cleanup/roles/list/export/import/rename
omk run list/show
omk proof show
omk hud --once/--json/--tui/--web
omk autopilot
omk ralph
omk ultrawork
```
