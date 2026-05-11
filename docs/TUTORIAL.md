# OMK Tutorial

This tutorial covers the current OMK MVP surface: Kimi-native assets,
scheduler-backed `team run`, HUD/run/proof inspection, and cleanup.

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
MOCK_KIMI=1 ./scripts/north_star_demo.sh
```

The mock path isolates `HOME`/`XDG_*`, creates a tiny failing Rust fixture,
repairs it deterministically, and expects a ready proof.

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
