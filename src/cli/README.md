# CLI Area Map

`src/cli/` contains command handlers and user-facing behavior. Keep this layer thin: parse command options, call runtime modules, format output, and return clear errors.

## Files

| File | Owns |
| --- | --- |
| `app.rs` | Top-level Clap app, logging setup, setup/update/completions/man/version dispatch. |
| `mod.rs` | CLI module exports. |
| `ask.rs` | Provider ask command. |
| `autopilot.rs` | Autopilot command entrypoint. |
| `backup.rs` | Backup list/create/restore commands. |
| `cleanup.rs` | Cleanup command behavior. |
| `config_cmd.rs` | Config validation and display. |
| `cost_cmd.rs` | Cost reporting command. |
| `doctor.rs` | Environment diagnostics. |
| `goal.rs` | Goal controller scaffold commands: plan/run/list/status/show/proof/replay/budget/budget-add/verify/execute/review/pause/resume/cancel, including local gates, replayable event timelines, budget checkpoints, wall-clock budget enforcement and recovery, the first bounded Wire-backed execution wave with mutation evidence, pause/resume lifecycle state, post-mutation gate reruns, and controller review/security evidence. |
| `hud.rs` | Text/JSON/TUI/web HUD command entrypoints. |
| `kimi_native_cmd.rs` | Kimi-native sync/install/doctor/assets commands. |
| `logs.rs` | Log inspection. |
| `marketplace.rs` | Marketplace command surface. |
| `proof_cmd.rs` | Proof/readiness command. |
| `ralph.rs` | Ralph command entrypoint. |
| `run_cmd.rs` | Run timeline/show command. |
| `skill.rs` | Skill management commands. |
| `state.rs` | State import/export/list commands. |
| `team.rs` | Current team run/list/status/rename/export/import/shutdown/health/cleanup/roles surface. |
| `team/proof.rs` | Team proof and failure-artifact finalization helpers. |
| `team/run_support.rs` | Kimi metadata, fallback subtasks, synthesis, and Wire worker setup helpers. |
| `ultrawork.rs` | Ultrawork command entrypoint. |

## Edit Rules

- Prefer calling `src/runtime/` or domain modules instead of adding business logic here.
- Keep `src/main.rs` as a thin binary wrapper over `omk::cli::run`; the CLI app lives in the library so integration tests can import it.
- Keep command output stable unless the task is explicitly about UX/help text.
- Update `README.md` and `docs/PROJECT_MAP.md` when adding or renaming a public command.
- Keep `team.rs` focused on command flow and output. Put proof/failure-artifact behavior in `team/proof.rs` and Wire run setup helpers in `team/run_support.rs`.

## Tests

Start with the command-specific integration test:

```bash
cargo test --test cli_smoke
cargo test --test kimi_native_test
cargo test --test proof_cmd_test
cargo test --test team_lifecycle_test
```

Use the narrowest test first, then widen to `cargo test` when command routing or shared output changes.
