# runtime::goal Agent Rules

## Module Architecture

This is a **subsystem** (level 2). It contains 20+ component modules.

### File Size Limits
- `mod.rs` must stay under 100 lines (storefront, not warehouse).
- `dispatch/mod.rs` and `tasks/mod.rs` are storefronts (~10 lines each); no proxy re-exports.
- Any file over 400 lines must be split (see root `AGENTS.md`).

### Import Rules
- **No `super::super::` imports.** Use `crate::runtime::goal::*` absolute paths.
- **No proxy re-exports.** Component modules import directly from their source,
  not via parent `mod.rs` convenience bundles.
- `runtime::goal` must not import `cli`, `vis`, `cost`, `notifications`, `mcp`,
  `skills`, `agents`, `marketplace`.

### State I/O Isolation
- `state/persistence.rs` owns all filesystem I/O for `GoalState`.
- All other modules treat `GoalState` as pure data.
- Query functions belong in `queries.rs`.

### Dispatch Boundary
- `lifecycle.rs` orchestrates; `dispatch/` executes agent task waves.
- `dispatch/` modules must not import `lifecycle` or `planner`.
