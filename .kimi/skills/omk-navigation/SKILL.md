---
name: omk-navigation
description: Use before editing oh-my-kimi when a task touches Rust modules, Kimi Wire integration, Kimi-native assets, team runtime, proof, HUD, or multiple files.
---

# OMK Navigation

Use this skill to reduce repository scanning and keep Kimi focused on the smallest useful context.

## First Reads

1. Read `docs/PROJECT_MAP.md`.
2. Read the area README for the files you expect to touch:
   - `src/cli/README.md`
   - `src/runtime/README.md`
   - `src/wire/README.md`
   - `src/kimi_native/README.md`
3. Run `scripts/repo-map.sh` if the task spans more than one area or the relevant files are unclear.

## Subagent Routing

- Use `explore` for read-only lookup, symbol mapping, and "where is this implemented?" questions.
- Use `plan` when a change touches multiple areas, changes runtime behavior, or needs a test strategy.
- Use `executor` only after the target files and verification commands are known.

Keep `explore` read-only. Do not ask the implementation agent to rediscover the whole repository.

## Kimi Integration Rules

Official Kimi docs are the source of truth:

- Docs root: https://www.kimi.com/code/docs
- Wire Protocol: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html
- Skills: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills.html
- Subagents: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/sub-agents.html

For Kimi process-control work, prefer `kimi --wire` and typed Wire events/requests. Prompt scraping is a fallback.

## Large-File Rule

Known large files include `src/cli/team.rs`, `src/wire/protocol.rs`, and `src/runtime/autopilot.rs`. Do not split them unless the user explicitly asks. Make the smallest local edit and leave broader refactors for a separate task.

## Verification

Pick the narrowest useful command first:

```bash
cargo fmt --check
cargo test --test wire_protocol_test
cargo test --test kimi_native_test
cargo test --test team_lifecycle_test
cargo test --test cli_smoke
```

For real Wire behavior, run `scripts/kimi-wire-smoke.sh` when a local authenticated Kimi CLI is available.
