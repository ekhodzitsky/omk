---
schema_version: 1
module: cli
level: root
purpose: CLI argument parsing, command dispatch, and human-facing output formatting for the omk binary
status: stable
surface:
  - name: run
    kind: fn
    visibility: pub
    contract: Library entrypoint for the CLI. Parses arguments, initializes tracing and state directories, dispatches to command handlers. Graceful shutdown on SIGINT/SIGTERM.
    proof:
      kind: integration-test
      target: cli_smoke
      command: "cargo test --test cli_smoke"
dependencies:
  internal:
    - module: runtime
      scope: app/run.rs, command handlers
      reason: State directory setup, event logging, scheduler dispatch, wire workers, gates, proof generation, worker management
    - module: kimi_native
      scope: kimi_native_cmd handlers
      reason: Asset sync, install, doctor, rollback, role packs, skills listing
    - module: mcp
      scope: app/run.rs (McpServer branch)
      reason: MCP server command dispatch
  external: []
consumers:
  - path: src/main.rs
    uses: [run]
  - path: tests/cli_smoke.rs
    uses: [run (via cargo_bin)]
  - path: tests/goal_end_to_end_cli_smoke_basic.rs
    uses: [run (via cargo_bin)]
  - path: tests/goal_end_to_end_cli_smoke_recovery.rs
    uses: [run (via cargo_bin)]
invariants:
  - id: thin-main
    rule: "src/main.rs only calls cli::run().await; no business logic in the binary crate."
    proof:
      kind: static-check
      target: src/main.rs
      command: "grep -c 'cli::run' src/main.rs"
  - id: no-direct-super-super
    rule: "No file in this module uses super::super:: imports."
    proof:
      kind: static-check
      target: src/cli/
      command: "! grep -r 'super::super::' src/cli/"
  - id: command-dispatch
    rule: "All CLI commands dispatch to library modules; no inline business logic in app/run.rs beyond routing and setup."
    proof:
      kind: static-check
      target: src/cli/app/run.rs
      command: "wc -l src/cli/app/run.rs"
verification:
  pre_change:
    - cargo run --bin omk -- --help
    - cargo run --bin omk -- --version
  full:
    - cargo test --test cli_smoke
    - cargo clippy --all-targets --all-features -- -D warnings
---

# cli

## Architecture

```
┌─────────────┐
│   main.rs   │  ← thin binary wrapper: cli::run().await
└──────┬──────┘
       │ pub fn run()
┌──────▼──────┐
│ cli::app    │  ← clap Parser (Omk) + Commands enum
│  /run.rs    │  ← dispatch match, tracing init, signal handling
└──────┬──────┘
       │
  ┌────┴────┬────────┬─────────┬──────────┐
  ▼         ▼        ▼         ▼          ▼
team    goal    autopilot  ralph     marketplace
ask     hud     doctor     cleanup   backup
config  state   skill      logs      cost
run_cmd proof   ultrawork  kimi_native
```

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Module declarations; exports `app::run`. |
| `app/mod.rs` | `Omk` clap Parser and `Commands` enum. |
| `app/run.rs` | Entrypoint `run()`, signal handling, tracing init, command dispatch. |
| `app/setup.rs` | `omk setup` implementation. |
| `app/update.rs` | `omk update` implementation. |
| `ask.rs` | `omk ask` command. |
| `autopilot.rs` | `omk autopilot` command. |
| `backup.rs` | `omk backup` command. |
| `cleanup.rs` | `omk cleanup` command + unit tests. |
| `config_cmd.rs` | `omk config` command. |
| `cost_cmd.rs` | `omk cost` command. |
| `doctor.rs` | `omk doctor` command. |
| `goal/` | `omk goal` subcommands (run, budget, integration, help, validate). |
| `hud.rs` | `omk hud` command. |
| `kimi_native_cmd/` | `omk kimi` subcommands (sync, install, doctor, agents, skills, hooks, rollback). |
| `logs.rs` | `omk logs` command. |
| `marketplace.rs` | `omk marketplace` command. |
| `proof_cmd.rs` | `omk proof` command. |
| `ralph.rs` | `omk ralph` command. |
| `run_cmd.rs` | `omk run` command. |
| `skill.rs` | `omk skill` command. |
| `state.rs` | `omk state` command. |
| `team/` | `omk team` subcommands (run, manage, inspect, proof, args, run_support). |
| `ultrawork.rs` | `omk ultrawork` command. |

## Edit Rules

- Keep `src/main.rs` as a thin binary wrapper over `omk::cli::run`; the CLI app lives in the library so integration tests can import it.
- Do not add business logic to `app/run.rs` beyond command routing, tracing setup, and signal handling.
- Respect the 400-line file limit; split command handlers into subdirectories when they grow.
- Feature-gate heavy dependencies (`vis` TUI/web) behind `tui` / `server` flags.
- Validate CLI input eagerly before performing side effects (see `goal/validate.rs`).

## Tests

```bash
cargo test --test cli_smoke
cargo test --lib cli::cleanup
cargo test --lib cli::goal::validate
cargo test --lib cli::team::proof
```
