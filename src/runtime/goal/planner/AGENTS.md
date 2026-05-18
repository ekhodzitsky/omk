# planner — Agent Guide

## Editing Rules

1. **Discovery is pure.** `discover.rs` must not perform I/O (network, DB, child
   processes). It only walks `std::fs` and parses with `tree-sitter`.
2. **Artifacts are append-only.** `artifacts.rs` writes planning markdown. Never
   overwrite existing artifact files without an explicit overwrite flag.
3. **Tree-sitter parsers are lazy.** Grammar loading is expensive; keep parsers
   in `OnceLock` or similar static initialization.
4. **Keyword matching is heuristic.** `discover.rs` scores by keyword overlap.
   Do not rely on it for security decisions; it is a planning hint only.
5. **Tests use temp directories.** All `discover.rs` tests must create temporary
   file trees. Do not depend on the real project layout.
