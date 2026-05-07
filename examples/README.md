# OMK Examples

## Basic Team Mode

Spawn 3 coder agents to fix TypeScript errors:

```bash
omk team spawn 3:coder "fix all TypeScript errors in src/"
```

Check status:

```bash
omk team status coder-abc123
```

Shut down when done:

```bash
omk team shutdown coder-abc123
```

## Autopilot

Build a complete REST API:

```bash
omk autopilot "build a Rust REST API for task management with CRUD endpoints"
```

With Ralph persistence:

```bash
omk autopilot --ralph "refactor the authentication module"
```

## Ralph Mode

Persistent refactoring with verification:

```bash
omk ralph "migrate from Express to Fastify"
```

## Cross-Provider Ask

Get a second opinion from Claude:

```bash
omk ask claude "review my database schema design"
```

Synthesize multiple advisors:

```bash
omk ask all "architecture for a real-time chat system"
```

## HUD

Attach tmux status bar:

```bash
# In ~/.tmux.conf
set -g status-right '#(omk hud --tmux)'
```

Interactive TUI:

```bash
omk hud --tui
```
