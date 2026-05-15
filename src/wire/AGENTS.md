# wire — Agent Guide

## Editing Rules

1. **Trait is the contract; struct is the implementation.** `WireClient` is a trait.
   `ProcessWireClient` is the only production implementation (via child process).
   `InMemoryWireClient` is for tests only.
2. **Do not add methods to `ProcessWireClient` without adding them to the trait.**
   If a consumer needs a new protocol method, it goes into the `WireClient` trait
   with a default implementation; `ProcessWireClient` implements only low-level
   primitives (`send_request`, `read_message`, `read_response`).
3. **process_messages is generic.** Do not bind the dispatch loop to a concrete type.
   Use `impl WireClient` or `<C: WireClient>`.
4. **No shell scripts in tests.** Any parsing, buffering, or dispatch logic is tested
   via `InMemoryWireClient`. `ProcessWireClient::spawn` is tested in one test only (smoke).
5. **Protocol facts must not go stale.** When changing protocol fields, update
   `README.md`, `docs/`, and tests in the same PR.
