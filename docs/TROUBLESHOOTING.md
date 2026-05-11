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
