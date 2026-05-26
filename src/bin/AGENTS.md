# Binary Crates — Agent Guide

## Editing Rules

1. **Thin binaries only.** Every binary in this directory must be a thin wrapper around library code. No business logic, no state management, no I/O orchestration.
2. **400-line limit.** If a binary grows beyond 400 lines, promote it to a standalone crate or a script under `scripts/`.
3. **Update README.md when adding binaries.** The `README.md` file map must stay accurate.
4. **No async runtime initialization.** Binaries that need Tokio should use `#[tokio::main]` from the library, not spawn their own runtime.
5. **Exit codes.** Use `std::process::exit` with documented codes (0 = success, 1 = validation failure, 2 = misuse).
