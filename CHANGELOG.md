# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `omk team status <name>` — inspect team state, workers, and task progress.
- `omk team shutdown <name>` — gracefully terminate a team session.
- Skill injection into lead prompt via `--skill` flag (default: `team`).
- Bundled skill loader (`load_bundled_skill`) from `CARGO_MANIFEST_DIR/skills/`.

### Changed
- `omk team` restructured to clap subcommands: `spawn`, `status`, `shutdown`.

## [0.1.0] - 2026-05-07

### Added
- Initial scaffold for `omk` CLI.
- `omk team <N:ROLE> <TASK>` — spawn N Kimi agents in tmux with JSONL IPC.
- `omk autopilot <TASK>` — scaffold for 6-phase autonomous execution.
- `omk ralph <TASK>` — scaffold for persistent verify/fix loops.
- `omk ask <PROVIDER> <PROMPT>` — cross-provider consultation scaffold.
- `omk hud` — statusline (`--tmux`) and TUI (`--tui`, requires `tui` feature).
- `omk setup` — initialize `~/.omk/` directory structure.
- Skill system: YAML frontmatter parser, skill discovery, bundled skills.
- Bundled skills: `team`, `autopilot`, `ralph`, `ultrawork`.
- Agent prompts: `executor`, `architect`, `critic`, `planner`.
- Runtime: tmux session/pane management, file-based bridge (inbox/outbox), worker lifecycle.
- Hook template for Kimi CLI `UserPromptSubmit`.
