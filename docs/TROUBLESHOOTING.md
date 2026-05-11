# Troubleshooting Guide

## Installation Issues

### `cargo install --git https://github.com/ekhodzitsky/oh-my-kimi.git` fails

**Symptom:** Compilation errors or missing dependencies.

**Solution:**
- Ensure Rust 1.78+ is installed: `rustc --version`
- OMK is not published to crates.io yet; install from GitHub Release assets or with `cargo install --git`.
- Install required system packages:
  ```bash
  # Ubuntu/Debian
  sudo apt-get install pkg-config libssl-dev

  # macOS
  brew install openssl
  ```

### `omk` command not found after installation

**Symptom:** `omk: command not found`

**Solution:**
- Check that `~/.cargo/bin` is in your PATH:
  ```bash
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
  source ~/.bashrc
  ```
- Or use the install script which handles this automatically.

## Runtime Issues

### Tmux not found

**Symptom:** `Error: tmux not found in PATH`

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install tmux

# macOS
brew install tmux

# Verify
which tmux
```

### Kimi CLI not found

**Symptom:** `Error: kimi not found in PATH`

**Solution:**
```bash
# Install Kimi CLI using the official upstream instructions:
# https://www.kimi.com/code/docs

# Then verify from the same shell where you run OMK:
kimi --version
which kimi
```

### Team spawn fails with "session already exists"

**Symptom:** `Error: tmux session already exists`

**Solution:**
```bash
# First try graceful OMK shutdown
omk team shutdown --force <name>

# If a tmux session is orphaned, remove only that session
tmux kill-session -t omk-team-<name>
```

### Autopilot/Ralph state corruption

**Symptom:** `Error: failed to parse state file`

**Solution:**
```bash
# Backup first
omk backup create

# Inspect state first
omk state list

# Dry-run cleanup first
omk cleanup --teams --dry-run

# Restore from backup if needed
omk backup list
omk backup restore <timestamp>
```

### Kimi auth mismatch (looks logged in, but workers fail)

**Symptom:** Team workers fail early even though `kimi` is installed.

**Solution:**
```bash
# Verify auth and binary path in the same shell
kimi auth status
which kimi

# Re-run environment diagnostics
omk doctor
```

If auth is not valid, complete the Kimi CLI login flow, then retry the team command.

### Kimi Wire initialize fails

**Symptom:** `omk team run` reports `Failed to parse initialize response` or fails before a worker turn begins.

**Solution:**
```bash
kimi info
cargo build --bin omk
```

Then compare the local protocol report with `docs/KIMI_UPSTREAM.md`. Kimi CLI 1.41.0 reports Wire protocol `1.9` and returns object-shaped hook metadata in `initialize.result.hooks`; new Kimi releases may add more extension fields.

### Stale state root confusion (`~/.omk/state` vs XDG)

**Symptom:** `omk run list` shows no runs, but files exist somewhere else.

**Why it happens:** OMK prefers `~/.omk/state` when `~/.omk/` exists; otherwise it uses `~/.local/state/omk`.

**Solution:**
```bash
ls -d ~/.omk/state ~/.local/state/omk 2>/dev/null
omk run list
omk state list
```

Use the active state root when checking logs/events manually.

### Kimi assets drift / stale `.kimi` workspace

**Symptom:** Team behavior does not match expected role/hook setup.

**Solution:**
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

**Symptom:** `Error: Address already in use`

**Solution:**
```bash
# Use a different port
omk hud --web --port 8081

# Or inspect the process using that port before stopping it
lsof -i :8080
```

## Performance Issues

### Slow team spawn

**Symptom:** Teams take a long time to initialize.

**Solution:**
- Check `kimi` CLI responsiveness: `time kimi --version`
- Reduce worker count: `omk team run 2:coder "task"` (or `team spawn` for compatibility mode)
- Check disk space in active state root:
  - `df -h ~/.local/state`
  - `df -h ~/.omk/state` (if legacy root is active)

### High memory usage

**Symptom:** System becomes sluggish when running multiple teams.

**Solution:**
- Limit concurrent teams
- Use `omk cleanup --older-than 1` to prune old state
- Monitor with `omk hud --web` and check `/api/metrics`

## Getting Help

1. Run diagnostics: `omk doctor`
2. Check logs: `cat ~/.local/state/omk/logs/omk.log`
3. Verify config: `omk config validate`
4. Open an issue with:
   - `omk --version` output
   - `omk doctor` output
   - Relevant log excerpts
