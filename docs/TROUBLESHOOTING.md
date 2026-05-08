# Troubleshooting Guide

## Installation Issues

### `cargo install omk` fails

**Symptom:** Compilation errors or missing dependencies.

**Solution:**
- Ensure Rust 1.78+ is installed: `rustc --version`
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
pip install kimi-cli
# or
pipx install kimi-cli
```

### Team spawn fails with "session already exists"

**Symptom:** `Error: tmux session already exists`

**Solution:**
```bash
# Kill the existing session
tmux kill-session -t omk-team-<name>

# Or use omk cleanup
omk cleanup --all
```

### Autopilot/Ralph state corruption

**Symptom:** `Error: failed to parse state file`

**Solution:**
```bash
# Remove corrupted state
rm -rf ~/.local/state/omk/autopilot/<name>
rm -rf ~/.local/state/omk/ralph/<name>

# Or restore from backup
omk backup list
omk backup restore <timestamp>
```

### Web dashboard port already in use

**Symptom:** `Error: Address already in use`

**Solution:**
```bash
# Use a different port
omk hud --web --port 8081

# Or kill the existing process
lsof -ti:8080 | xargs kill -9
```

## Performance Issues

### Slow team spawn

**Symptom:** Teams take a long time to initialize.

**Solution:**
- Check `kimi` CLI responsiveness: `time kimi --version`
- Reduce worker count: `omk team spawn 2:coder "task"`
- Check disk space: `df -h ~/.local/state`

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
