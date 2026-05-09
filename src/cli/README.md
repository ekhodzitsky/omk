# CLI Area Map

`src/cli/` contains command handlers and user-facing behavior. Keep this layer thin: parse command options, call runtime modules, format output, and return clear errors.

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | CLI module exports. |
| `ask.rs` | Provider ask command. |
| `autopilot.rs` | Autopilot command entrypoint. |
| `backup.rs` | Backup list/create/restore commands. |
| `cleanup.rs` | Cleanup command behavior. |
| `config_cmd.rs` | Config validation and display. |
| `cost_cmd.rs` | Cost reporting command. |
| `doctor.rs` | Environment diagnostics. |
| `hud.rs` | Tmux/TUI/web HUD command entrypoints. |
| `kimi_native_cmd.rs` | Kimi-native sync/install/doctor/assets commands. |
| `logs.rs` | Log inspection. |
| `marketplace.rs` | Marketplace command surface. |
| `proof_cmd.rs` | Proof/readiness command. |
| `ralph.rs` | Ralph command entrypoint. |
| `run_cmd.rs` | Run timeline/show command. |
| `skill.rs` | Skill management commands. |
| `state.rs` | State import/export/list commands. |
| `team.rs` | Current team spawn/run/list/status/attach/broadcast/shutdown surface. |
| `ultrawork.rs` | Ultrawork command entrypoint. |

## Edit Rules

- Prefer calling `src/runtime/` or domain modules instead of adding business logic here.
- Keep command output stable unless the task is explicitly about UX/help text.
- Update `README.md` and `docs/PROJECT_MAP.md` when adding or renaming a public command.
- `team.rs` is intentionally left unsplit for now. Touch the smallest relevant function.

## Tests

Start with the command-specific integration test:

```bash
cargo test --test cli_smoke
cargo test --test kimi_native_test
cargo test --test proof_cmd_test
cargo test --test team_lifecycle_test
```

Use the narrowest test first, then widen to `cargo test` when command routing or shared output changes.
