# cli — Agent Guide

## Editing Rules

1. **Thin dispatch only.** `src/main.rs` calls `omk::cli::run().await` and
   nothing else. Command handlers parse args and delegate to `runtime/` or
   `wire/` primitives. No business logic in the CLI layer.
2. **Exit codes are a contract.** `--help` returns 0. Missing required args
   return 2. Runtime failures return 1. Smoke tests assert the exact code.
3. **Human output goes to stderr, machine output to stdout.** JSON, proof, and
   replay text are written to stdout. Progress spinners, warnings, and errors
   go to stderr.
4. **CLI UX is testable.** New commands or flags require `--help` smoke
   coverage and at least one integration test for the happy path.
5. **File size limit applies.** Any `.rs` file under `src/cli/` exceeding 400
   lines must be split into a directory module (`foo.rs` → `foo/mod.rs`).
