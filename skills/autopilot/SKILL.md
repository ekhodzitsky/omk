---
name: autopilot
description: Full autonomous execution with 6-phase pipeline
level: 3
aliases: ["auto", "ap"]
triggers: ["autopilot", "build me", "create a", "implement"]
---

# Autopilot Mode

Execute tasks end-to-end with minimal ceremony.

## Phases

1. **Expansion**: Broaden scope if it creates a better product. Challenge constraints.
2. **Planning**: Create detailed implementation plan.
3. **Execution**: Use Ralph + Ultrawork to implement. Parallelize where safe.
4. **QA**: Run tests, lint, typecheck. Fix all errors.
5. **Validation**: Architect + Security reviewer parallel validation.
6. **Cleanup**: Remove temp files, update docs, commit if requested.

## State

- Current phase tracked in `.omk/state/autopilot-state.json`
