# Binary Crates

This directory contains auxiliary binary crates that ship alongside the main `omk`
CLI. The main binary (`src/main.rs`) must remain a thin dispatch layer that calls
`omk::cli::run().await` and nothing else.

## Binaries

| File | Purpose |
|------|---------|
| `validate-contracts.rs` | Walks `src/` and validates that every module `README.md` contains the required YAML frontmatter contract (`schema_version`, `module`, `level`, `purpose`, `status`, `surface`, `dependencies`, `consumers`, `invariants`, `verification`). Used in CI to enforce module documentation hygiene. |

## Edit Rules

- Do not add business logic here. Binaries are tooling-only.
- Keep each binary under 400 lines. If a tool grows larger, promote it to a
  standalone crate or script under `scripts/`.
- Update this README when adding a new binary.
