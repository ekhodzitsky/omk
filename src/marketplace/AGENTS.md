# marketplace — Agent Guide

## Editing Rules

1. **Registry loading is pure.** `load_all_skills` accepts `&[RegistrySource]`
   and returns `(registry_name, skill)` tuples. Network or filesystem I/O must
   happen through a `RegistryLoader` trait, not directly in `load_all_skills`.
2. **Failures are non-fatal.** A bad registry URL or malformed JSON must log and
   skip, never bail the entire call.
3. **No write side effects.** Marketplace only reads registries. If a feature
   needs writes (publishing), it belongs in a new `marketplace/publish.rs` with
   its own trait boundary.
4. **Cache-aware.** Registries may be large. Prefer streaming JSON parse or
   bounded memory buffers over loading the entire response into a `String`.
5. **Test through mocks.** `MockRegistryLoader` lives in `loader.rs` and is tested alongside registry merge logic.
   All registry merge logic is tested without network or temp files.
