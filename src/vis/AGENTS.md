# vis — Agent Guide

## Editing Rules

1. **Renderers are pure functions.** `render_goal_progress`, `render_budget`,
   and similar functions take typed state and return `String`. No I/O, no
   global state, no `Utc::now()` inside renderers.
2. **Output is machine-readable or human-readable, never both mixed.** JSON
   renderers produce valid serde structs. Text renderers produce deterministic
   lines. Do not embed chat-style prefixes ("assistant:", "user:") in any view.
3. **EventStream is the only file reader.** All other vis modules receive
   already-loaded state. `EventStream` handles truncation, malformed lines, and
   incremental polling in one place.
4. **Color is opt-in.** Terminal color codes are controlled by `--color` or
   `NO_COLOR`. Default to plain text when piping.
5. **Test through fixtures.** Golden fixtures live in `tests/fixtures/vis/`.
   Render output is compared verbatim; any formatting change updates the fixture.
