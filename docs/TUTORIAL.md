# oh-my-kimi Tutorial

## Table of Contents

1. [Installation](#installation)
2. [Your First Team](#your-first-team)
3. [Autopilot Mode](#autopilot-mode)
4. [Cross-Provider Consulting](#cross-provider-consulting)
5. [Web Dashboard](#web-dashboard)
6. [Skill Management](#skill-management)
7. [State Management](#state-management)
8. [Troubleshooting](#troubleshooting)

## Installation

### Via cargo

```bash
cargo install omk
```

### Self-update

```bash
omk update --check   # Check for updates
omk update           # Install latest release
```

### Via install script

```bash
curl -fsSL https://raw.githubusercontent.com/ekhodzitsky/oh-my-kimi/master/install.sh | bash
```

### Verify installation

```bash
omk doctor
omk setup
```

## Your First Team

### Spawn a team

```bash
omk team spawn 3:coder "refactor authentication to use JWT tokens"
```

Output:
```
✓ Team 'coder-a1b2' started with 3 coder worker(s)
  Session: omk-team-coder-a1b2
  State:   ~/.local/state/omk/team/coder-a1b2

Commands:
  omk team status coder-a1b2
  omk team shutdown coder-a1b2

Attach with: omk team attach coder-a1b2
```

### List all teams

```bash
omk team list
```

### Check status

```bash
omk team status coder-a1b2
```

### Attach to tmux session

```bash
omk team attach coder-a1b2
# or directly with tmux:
tmux attach -t omk-team-coder-a1b2
```

### Broadcast a message

```bash
omk team broadcast coder-a1b2 "New requirement: add OAuth support"
```

### Shutdown

```bash
omk team shutdown coder-a1b2
```

## Autopilot Mode

### Run full pipeline

```bash
omk autopilot "build a REST API for task management"
```

### Resume after interruption

```bash
omk autopilot --resume --name ap-xxx "build a REST API"
```

### YOLO mode (continue on failures)

```bash
omk autopilot --yolo "migrate from Express to Fastify"
```

### With Ralph persistence

```bash
omk autopilot --ralph "implement user authentication"
```

## Cross-Provider Consulting

### Ask a single provider

```bash
omk ask claude "review my API design"
```

### Ask all providers

```bash
omk ask all "architecture for real-time chat"
```

### Ask specific providers

```bash
omk ask --providers claude,kimi "database schema review"
```

### Disable synthesis

```bash
omk ask all --no-synthesis "compare Rust vs Go for microservices"
```

### Adjust timeout

```bash
omk ask all --timeout 120 "complex distributed systems question"
```

## Web Dashboard

### Start dashboard

```bash
omk hud --web --port 8080
```

### Open browser

Navigate to `http://localhost:8080`

The dashboard shows:
- Active teams with phases and tasks
- Running autopilots and their current phase
- Ralph sessions with iteration progress
- Metrics (spawns, shutdowns, tasks, ask calls)
- Health status and version

### Docker Compose

```bash
docker-compose up -d
```

## Skill Management

### Browse marketplace

```bash
omk marketplace list
```

### Use an external registry

```bash
omk marketplace add-registry https://example.com/registry.json
omk marketplace list-registries
omk marketplace list
```

### Show skill info

```bash
omk marketplace info rust-expert
```

### Install from marketplace

```bash
omk marketplace install rust-expert
```

### Install from a specific registry

```bash
omk marketplace install my-skill --registry https://example.com/registry.json
```

### Install from git

```bash
omk skill install https://github.com/user/omk-skill-repo
```

### List installed skills

```bash
omk skill list
```

### Show a skill's contents

```bash
omk skill show rust-expert
```

### Search installed skills

```bash
omk skill search rust
```

### Remove a skill

```bash
omk skill remove rust-expert
```

### Remove a registry

```bash
omk marketplace remove-registry https://example.com/registry.json
```

## State Management

### Create backup

```bash
omk backup create
```

### List backups

```bash
omk backup list
```

### Restore from backup

```bash
omk backup restore 20260508-121530
```

### Prune old backups

```bash
omk backup prune --keep 5
```

### List all sessions

```bash
omk state list
```

### Export state as JSON

```bash
omk state export --output my-project-state.json
```

### Import state

```bash
omk state import --input my-project-state.json
```

### Clean up old state

```bash
omk cleanup --older-than 7
omk cleanup --artifacts --older-than 30
```

## Troubleshooting

### Check environment

```bash
omk doctor
omk config validate
omk config show
omk config set default_team_size 3
```

### View logs

```bash
cat ~/.local/state/omk/logs/omk.log
```

### Reset everything

```bash
omk cleanup --all
omk setup
```
