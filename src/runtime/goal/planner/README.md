# planner

Goal planning and file discovery for `omk goal`.

## Purpose

Generates durable planning artifacts (goal brief, technical plan, test spec) and
performs AST-based file discovery to find semantically relevant source files.

## Public API

- `discover_relevant_files(goal, project_dir) -> Result<Vec<PathBuf>>` — walks
  the project, parses source with tree-sitter, and returns files whose function
  names overlap with goal keywords.
- `write_goal_brief`, `write_technical_plan`, `write_test_spec` — artifact
  generators that write markdown into the goal state directory.

## Consumers

- `src/runtime/goal/` — lifecycle and delivery
- `src/runtime/goal/planner/scaffold.rs` — orchestrates artifact generation

## Status

Experimental. `discover_relevant_files` supports Rust, JavaScript, TypeScript,
Python, and Go.

## Dependencies

- `tree-sitter` + language grammars
- `crate::analysis` (parse helpers)
- `crate::runtime::goal::state`
