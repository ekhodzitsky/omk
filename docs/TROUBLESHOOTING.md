# Troubleshooting Guide

## Installation Issues

### `cargo install --git https://github.com/ekhodzitsky/oh-my-kimi.git` fails

Ensure Rust 1.78+ is installed:

```bash
rustc --version
```

OMK is not published to crates.io yet. Use GitHub Release assets or
`cargo install --git`.

On Linux, install the usual build dependencies if OpenSSL/pkg-config errors
appear:

```bash
sudo apt-get install pkg-config libssl-dev
```

### `omk` command not found

Check that the install location is on your `PATH`:

```bash
echo "$PATH"
ls ~/.cargo/bin/omk ~/.local/bin/omk 2>/dev/null
```

The GitHub install script handles this automatically for common shells.

## Runtime Issues

### Kimi CLI not found

Verify from the same shell where you run OMK:

```bash
kimi --version
which kimi
omk doctor
```

Install and authenticate Kimi CLI using the official upstream docs:
https://www.kimi.com/code/docs

### Kimi auth mismatch

Workers may fail early when the `kimi` binary exists but auth is not valid for
the current shell.

```bash
kimi auth status
which kimi
omk doctor
```

Complete the Kimi login flow, then retry the team command.

### Kimi Wire initialize fails

Symptoms include parse errors before a worker turn begins.

```bash
kimi info
cargo build --bin omk
```

Compare the local protocol report with `docs/KIMI_UPSTREAM.md`. Kimi CLI
versions may add extension fields in `initialize.result`; OMK should parse those
as structured JSON evidence rather than a closed schema.

### Team run hangs or looks stuck

Start with the CLI views:

```bash
omk team health <team-name>
omk run show latest
omk proof show latest
```

Then inspect the state files:

```bash
ls ~/.local/state/omk/team/<team-name>
ls ~/.local/state/omk/team/<team-name>/workers
```

If you use the legacy state root, inspect `~/.omk/state/team/<team-name>`
instead. OMK prefers `~/.omk/state` when `~/.omk/` already exists; otherwise it
uses `~/.local/state/omk`.

### Run has no proof

Check whether the run wrote a failure artifact:

```bash
omk run show latest
omk proof show latest
find ~/.local/state/omk/team -name failure.json -o -name proof.json
```

Failed or interrupted runs should produce `failure.json`. Not-ready proof output
usually means a required verification gate failed or never ran.

### Greenfield goal proof stays `not_ready`

This is expected for the current `omk goal` MVP until all evidence exists. Check
the proof first:

```bash
omk goal show latest
omk goal proof latest --format md
omk goal replay latest --format text
```

Common missing evidence:

- no local gates ran;
- required gates failed;
- `omk goal execute latest` has not run;
- `omk goal review latest` has not attached review and security evidence;
- agent changes exist, but the integration loop has not accepted, committed, or
  opened a PR for them.

The proof distinguishes **engineering-ready evidence** from **product-ready
release acceptance**. Passing gates plus agent/review evidence means the result
is ready for engineering handoff. Product readiness still requires human
acceptance, PR/release work, and any product or positioning decisions.

### Greenfield goal has no gates

`omk goal verify` auto-detects gates from project files. A blank directory has
no reliable oracle, so the proof records the gap instead of pretending success.
For the greenfield acceptance demo, start with a tiny project fixture:

```bash
cargo new omk-goal-greenfield-demo
cd omk-goal-greenfield-demo
omk setup
omk goal run "Build a tiny local-only Rust CLI with add/list commands and tests" --max-agents 1
omk goal verify latest
```

For non-Rust projects, add a project-native manifest (`package.json`,
`pyproject.toml`, `go.mod`) or define explicit gates in `.omk/gates.toml`.

### `omk goal execute` cannot start workers

Goal execution needs a Wire-capable Kimi runtime:

```bash
kimi --version
kimi auth status
omk goal execute latest
```

For offline tests, `MOCK_KIMI` must point at an executable wire-compatible mock,
not just be set to arbitrary text:

```bash
MOCK_KIMI=/path/to/mock-kimi-wire omk goal execute latest
```

If Kimi is unavailable, `goal run`, `goal show`, `goal verify`, `goal proof`,
and `goal replay` still produce useful planning and gate artifacts, but the
proof should remain `not_ready` because bounded agent execution evidence is
missing.

### Goal blocked on human oracle

Vague requests such as "make this app great" or "build a product users love"
can stop as `blocked_on_human`. Rewrite the goal with testable behavior,
explicit constraints, and gates:

```bash
omk goal run "Build a local-only Rust CLI named taskline with add/list commands, tasks.txt storage, command tests, no network access, and no new dependencies"
```

If the goal depends on taste, pricing, legal review, credentials, or external
business judgment, capture that as a human decision before expecting autonomous
execution to continue.

### Goal rejected by the integrator

`omk goal reject latest --reason <text>` keeps the proof `not_ready`, records
`integration_evidence.status = rejected`, and writes a rollback-plan artifact
under the goal's `artifacts/integration/` directory. Inspect it before starting
the next slice:

```bash
omk goal proof latest --json
omk goal show latest
```

The next attempt should either revert the rejected changed-file scope or replace
it in a new task-scoped branch/worktree, then rerun verify, execute, review, and
acceptance.

### Goal needs more budget

When wall-clock, token, or USD limits are exhausted, the goal status becomes
`needs_more_budget` instead of silently continuing:

```bash
omk goal budget latest
omk goal budget-add latest --time 1h
omk goal budget-add latest --tokens 500000 --usd 5
```

Budget extensions are explicit operator decisions and are recorded in
`budget-checkpoints.jsonl`.

### Kimi assets drift

If agent behavior does not match expected roles/hooks/skills:

```bash
omk kimi doctor
omk kimi sync --dry-run
omk kimi sync
```

If sync changed the wrong files:

```bash
omk kimi rollback --dry-run
omk kimi rollback
```

### Web dashboard port already in use

Use another port or inspect the existing process:

```bash
omk hud --web --port 8081
lsof -i :8080
```

## Performance Issues

### Slow team run

- Check Kimi CLI responsiveness: `time kimi --version`.
- Reduce worker count: `omk team run 1:executor "task"`.
- Check free disk space for the active state root.

```bash
df -h ~/.local/state
df -h ~/.omk/state
```

### High memory usage

- Limit concurrent teams.
- Prefer smaller worker counts until the task truly benefits from parallelism.
- Prune old state after a dry run:

```bash
omk cleanup --teams --dry-run
omk team cleanup --dry-run --older-than 7
```

## Getting Help

1. Run diagnostics: `omk doctor`
2. Check run evidence: `omk run show latest`
3. Check proof evidence: `omk proof show latest`
4. Open a GitHub issue with:
   - `omk --version`
   - `omk doctor`
   - the command you ran
   - relevant `events.jsonl`, `proof.json`, or `failure.json` excerpts
