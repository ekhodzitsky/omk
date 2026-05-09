# OMK Examples

## Kimi Team Run

Run 3 coder workers to fix TypeScript errors:

```bash
omk team run 3:coder "fix all TypeScript errors in src/"
```

Check status:

```bash
omk team status coder-abc123
```

Shut down when done:

```bash
omk team shutdown coder-abc123
```

Inspect the run:

```bash
omk proof show latest
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

## Kimi Ask

Get a second opinion from Kimi:

```bash
omk ask kimi "review my database schema design"
```

Multi-provider advisor flows are later. Keep the examples Kimi-first.

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

## Killer Demo Fixture

CI-safe scripted fixture with deterministic proof/demo output:

```bash
examples/killer-demo/run.sh
```

Relation to North Star demo:
- `examples/killer-demo` is the deterministic CI fixture contract (`demo-output.txt` snapshot surface).
- `scripts/north_star_demo.sh` is the operator-facing smoke flow; in `MOCK_KIMI=1` mode it follows the same mock-first isolation guarantees and now expects a green proof after deterministic fixture repair.
