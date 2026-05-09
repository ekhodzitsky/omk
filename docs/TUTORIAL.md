# oh-my-kimi Tutorial

A step-by-step guide for new users. This tutorial covers **Current** commands only. Commands marked **Next** or **Later** are noted explicitly so you know what is available today and what is coming.

## Table of Contents

1. [Before You Start](#before-you-start)
2. [Installation and Setup](#installation-and-setup)
3. [Syncing Kimi Assets](#syncing-kimi-assets)
4. [Your First Team](#your-first-team)
5. [Watching Your Team](#watching-your-team)
6. [Proof and Run Inspection](#proof-and-run-inspection)
7. [Autopilot, Ralph, and Ultrawork](#autopilot-ralph-and-ultrawork)
8. [Skills and Marketplace](#skills-and-marketplace)
9. [Maintenance](#maintenance)
10. [Troubleshooting](#troubleshooting)
11. [Command Maturity Cheat Sheet](#command-maturity-cheat-sheet)

---

## Before You Start

You will need:

- **Rust 1.78+** (to build from source) or a published `omk` binary
- **Kimi CLI** installed and authenticated: `kimi --version` and `kimi auth status`
- **tmux** installed: `brew install tmux` or `apt install tmux`

Verify the basics:

```bash
omk doctor
omk setup
```

## Current vs Target (L8) At a Glance

Use this as a quick reality check while reading the tutorial:

- **Current (implemented now):** `omk team run`, `omk team spawn`, `omk kimi sync/doctor/install/rollback`, `omk run list/show`, `omk proof show`.
- **Current Scaffold (exists, still hardening):** `omk hud --web`, deeper run/proof filtering and UX polish.
- **Target (near-term direction):** stronger Wire-first proof/HUD ergonomics and fewer tmux-bridge edge cases.

For protocol and upstream truth, re-check [KIMI_UPSTREAM.md](KIMI_UPSTREAM.md) before relying on stale assumptions.

---

## Installation and Setup

### Install `omk`

```bash
# Via cargo
cargo install omk

# Or via install script
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash
```

### Initialize your environment

```bash
omk setup
```

This creates OMK config and state directories (`~/.config/omk/` and `~/.local/state/omk/`).

### Validate everything

```bash
omk doctor
```

`omk doctor` checks that `kimi`, `tmux`, and required paths are available. If anything is missing, it tells you what to install.

### Inspect and change config

```bash
omk config show
omk config validate
omk config set default_team_size 3
```

---

## Syncing Kimi Assets

`omk kimi sync` copies OMK Kimi-native assets (agents, hooks, skills) into your project so Kimi CLI can use them. It also writes a project manifest that tracks what was installed.
If you are changing Kimi integration behavior, re-check [KIMI_UPSTREAM.md](KIMI_UPSTREAM.md) before relying on upstream assumptions.

### Run a sync

```bash
omk kimi sync
```

What it does:
- Copies agents, hooks, and skills into `.kimi/`
- Writes a project manifest (`.kimi/omk-manifest.json`)
- Backs up existing files before overwriting

### Preview changes with `--dry-run`

```bash
omk kimi sync --dry-run
```

This prints what would be copied, overwritten, or created without touching any files. Use it before syncing in a project with existing Kimi assets.

### Validate after syncing

```bash
omk kimi doctor
```

`omk kimi doctor` inspects `.kimi/` and reports:
- Missing required files
- Stale or unreferenced assets
- Permission issues
- Manifest drift

Run it after every sync to confirm the project is in a healthy state.

### Rollback if something goes wrong

```bash
omk kimi rollback --dry-run   # preview
omk kimi rollback             # actually restore backups
```

`omk kimi rollback` restores the backups that `omk kimi sync` created before overwriting files. Always use `--dry-run` first.

### Other Kimi-native commands

```bash
omk kimi install          # Install role agents, hooks, and skills
omk kimi agents           # List installed agents
omk kimi hooks            # List configured hooks
omk kimi skills           # List linked skills
```

---

## Your First Team

`omk team run` is the current scheduler-backed way to start a visible swarm of Kimi workers. `omk team spawn` remains available as the tmux-bridge compatibility path. Each worker runs in its own tmux pane, and OMK tracks state in JSONL inbox/outbox files.

### Run a scheduler-backed team

```bash
omk team run 3:coder "refactor authentication to use JWT tokens"
```

This is the current wire-first path. OMK owns claims, leases, watchdogs, and proof-oriented state for the run.

### Spawn a tmux compatibility team

```bash
omk team spawn 3:coder "refactor authentication to use JWT tokens"
```

Output:

```text
✓ Team 'coder-a1b2' started with 3 coder worker(s)
  Session: omk-team-coder-a1b2
  State:   ~/.local/state/omk/team/coder-a1b2

Commands:
  omk team status coder-a1b2
  omk team shutdown coder-a1b2

Attach with: omk team attach coder-a1b2
```

- `3:coder` means 3 workers using the `coder` role pack.
- The task description is sent to the lead worker.
- OMK creates a tmux session named `omk-team-coder-a1b2`.

### List active teams

```bash
omk team list
```

### Check team status

```bash
omk team status coder-a1b2
```

This prints worker heartbeats, which panes are running, and whether any workers have reported results.

### Attach to the tmux session

```bash
omk team attach coder-a1b2
```

You are now inside tmux. You can watch workers progress in real time.

**tmux tips for beginners:**

| Key | Action |
| --- | --- |
| `Ctrl+b` then `o` | Switch to next pane |
| `Ctrl+b` then `q` | Show pane numbers, then press a number to jump |
| `Ctrl+b` then `c` | Create a new window |
| `Ctrl+b` then `n` | Next window |
| `Ctrl+b` then `d` | Detach (return to shell without stopping the team) |
| `tmux ls` | List all tmux sessions from your normal shell |

To re-attach later:

```bash
omk team attach coder-a1b2
# or directly:
tmux attach -t omk-team-coder-a1b2
```

### Broadcast a message

```bash
omk team broadcast coder-a1b2 "New requirement: add OAuth2 support"
```

This sends text to every worker pane at once.

### Shutdown a team

```bash
omk team shutdown coder-a1b2
```

This kills the tmux session and cleans up running worker processes. If a team is stuck, use `--force`:

```bash
omk team shutdown coder-a1b2 --force
```

### What is `omk team run`?

`omk team run` is the current scheduler-backed entrypoint. It adds:
- Scheduler-owned task claims and leases
- Automatic watchdog recovery for stuck workers
- Event-log driven proof generation
- Wire-protocol first worker dispatch

Use `omk team spawn` when you need the older tmux bridge or compatibility behavior.

---

## Watching Your Team

The HUD shows what your teams are doing without attaching to tmux.

### tmux status bar string

```bash
omk hud --tmux
```

Prints a one-line status string you can embed in your tmux status bar. Add it to `~/.tmux.conf`:

```bash
set -g status-right '#(omk hud --tmux)'
```

### Live TUI

```bash
omk hud --tui coder-a1b2
```

Opens an interactive terminal UI that refreshes as workers report status. Press `q` to quit.
`--tui` requires a team name.

### Web dashboard (Scaffold)

```bash
omk hud --web --port 8080
```

Starts a local web dashboard. Open `http://localhost:8080` in a browser. This is **Current Scaffold**; the timeline and richer runtime visibility are still being hardened.

### One-shot snapshot

```bash
omk hud --once --json coder-a1b2
```

Prints a single JSON snapshot of the named team and exits. Useful for scripts.

---

## Proof and Run Inspection

OMK records runs so you can inspect what happened after the fact.

### List recorded runs

```bash
omk run list
```

### Show a run timeline

```bash
omk run show latest
omk run show <run-id>
```

This prints the event timeline for a run. Use `--format json` for programmatic consumption:

```bash
omk run show latest --format json
```

### Generate a proof report

```bash
omk proof show latest
omk proof show <run-id>
```

Generates a readiness report from the run's event log. The proof includes:
- **Status** — `Ready`, `NotReady`, or `Failed`
- **Changed files** — files modified during the run
- **Gates** — verification results (fmt, clippy, test) where available
- **Failures** — any worker or gate failures
- **Retries** — tasks that were retried
- **Known gaps** — explicitly acknowledged incomplete work

Output formats:

```bash
omk proof show latest --format text
omk proof show latest --format json
omk proof show latest --format md
```

> **Maturity note:** `omk run show` and `omk proof show` are **Current Scaffold**. They exist in the CLI today, but deeper timeline filtering, gate integration, and proof regeneration are still being hardened.

---

## Autopilot, Ralph, and Ultrawork

These are single-lead modes that do not spawn a tmux team.

### Autopilot

```bash
omk autopilot "build a REST API for task management"
```

Runs a six-phase autonomous execution. Resume after interruption:

```bash
omk autopilot --resume --name ap-xxx "build a REST API"
```

YOLO mode continues on failures:

```bash
omk autopilot --yolo "migrate from Express to Fastify"
```

### Ralph

```bash
omk ralph "migrate from Express to Fastify"
```

A persistent verify/fix loop. Limit iterations:

```bash
omk ralph --max-iterations 5 "update all dependencies"
```

### Ultrawork

```bash
omk ultrawork "fix all TypeScript errors"
```

Parallel burst execution without a tmux team. See `omk ultrawork --help` for flags.

---

## Skills and Marketplace

### Browse the marketplace

```bash
omk marketplace list
```

### Install a skill

```bash
omk marketplace install rust-expert
```

### Manage installed skills

```bash
omk skill list
omk skill show rust-expert
omk skill search rust
omk skill remove rust-expert
```

---

## Maintenance

### Backups

```bash
omk backup create
omk backup list
omk backup restore 20260508-121530
```

### State export and import

```bash
omk state export --output my-state.json
omk state import --input my-state.json
```

### Cleanup

```bash
omk cleanup --older-than 7
omk cleanup --artifacts --older-than 30
```

### Cost estimation

```bash
omk cost
```

Shows heuristic cost estimates across modes.

---

## Troubleshooting

### `kimi not found`

Install and authenticate [Kimi CLI](https://github.com/MoonshotAI/kimi-cli):

```bash
kimi --version        # should print version
kimi auth status      # should show you are logged in
```

### `tmux not found`

```bash
# macOS
brew install tmux

# Ubuntu/Debian
sudo apt install tmux
```

### `omk team spawn` hangs or fails

1. Run `omk doctor` and fix any red checks.
2. Verify Kimi auth: `kimi auth status`
3. Verify tmux: `tmux ls`
4. Check worker heartbeats:
   ```bash
   cat ~/.local/state/omk/team/<name>/workers/*/heartbeat.json
   ```
5. If a session already exists, kill it:
   ```bash
   tmux kill-session -t omk-team-<name>
   ```

### Kimi auth looks valid but workers still fail

1. Re-check auth state from the same shell OMK is using:
   ```bash
   kimi auth status
   which kimi
   ```
2. If status is not authenticated, complete the Kimi CLI login flow, then re-run:
   ```bash
   omk doctor
   ```
3. Re-run a minimal command to validate Kimi path/auth quickly:
   ```bash
   kimi --version
   ```

### tmux session exists but OMK team state is stale

1. Inspect active teams and sessions:
   ```bash
   omk team list
   tmux ls | rg omk-team-
   ```
2. Prefer OMK cleanup first:
   ```bash
   omk team shutdown --force <name>
   ```
3. If tmux session remains orphaned, remove it directly:
   ```bash
   tmux kill-session -t omk-team-<name>
   ```

### Kimi-native assets look stale

```bash
omk kimi doctor
omk kimi sync --dry-run
omk kimi sync
```

### Sync overwrote something I needed

```bash
omk kimi rollback --dry-run
omk kimi rollback
```

### State corruption

1. Create a backup before destructive cleanup:
   ```bash
   omk backup create
   ```
2. Check which state root OMK is using (`~/.omk/state` takes priority if `~/.omk/` exists):
   ```bash
   ls -d ~/.omk/state ~/.local/state/omk 2>/dev/null
   ```
3. Start with dry-run cleanup:
   ```bash
   omk team cleanup --all --dry-run
   omk cleanup --teams --dry-run
   ```
4. Apply cleanup only after review:
   ```bash
   omk team cleanup --all
   omk cleanup --teams
   ```
5. Re-run `omk setup`.

### Resume after crash

Use mode-specific `--resume` flags where available (`omk autopilot --resume`, `omk ralph --resume`).

### Slow team spawn

- Check `kimi` responsiveness: `time kimi --version`
- Reduce worker count: `omk team spawn 2:coder "task"`
- Check disk space: `df -h ~/.local/state`

---

## Command Maturity Cheat Sheet

| Label | Meaning |
| --- | --- |
| **Current** | Implemented in the CLI today. |
| **Current MVP** | Usable, but still needs hardening and real-world validation. |
| **Current Scaffold** | Command exists, but deeper integration is incomplete. |
| **Next** | Planned for the Kimi-only killer demo. Not available yet. |
| **Later** | Deferred until the Kimi-only runtime is excellent. |

### Current commands (today)

- `omk setup`, `omk doctor`, `omk config show/validate/set`
- `omk kimi sync`, `omk kimi doctor`, `omk kimi install`, `omk kimi agents`, `omk kimi hooks`, `omk kimi skills`, `omk kimi rollback`
- `omk team run`, `omk team spawn`, `omk team list`, `omk team status`, `omk team attach`, `omk team broadcast`, `omk team shutdown`
- `omk autopilot`, `omk ralph`, `omk ultrawork`
- `omk hud --tmux`, `omk hud <team-name> --tui`, `omk hud --web`
- `omk proof show <id\|latest>`, `omk run show <id\|latest>`, `omk run list`
- `omk ask`, `omk marketplace`, `omk skill`, `omk backup`, `omk state`, `omk cleanup`, `omk cost`

### Next commands

- None. The scheduler-backed team run path and proof show path are current.

### Later commands (deferred)

- Provider-neutral workers and cross-provider advisor flows remain Later.

---

For the North Star target workflow, see [north_star_tutorial.md](north_star_tutorial.md).
For project navigation, see [PROJECT_MAP.md](PROJECT_MAP.md).
