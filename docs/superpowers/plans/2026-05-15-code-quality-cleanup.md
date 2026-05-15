# Code Quality Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all `unwrap()`, `expect()`, and `panic!()` from production code in `src/`, then split files exceeding 400 lines into directory modules per SRP. Zero behavioral changes, all tests passing.

**Architecture:** Module-by-module mechanical refactoring. Phase 1 = error handling cleanup. Phase 2 = file splitting. Each task is self-contained and testable.

**Tech Stack:** Rust, tokio, anyhow/thiserror, cargo test, cargo clippy.

**Branch:** `refactor/code-quality-cleanup`

---

## Phase 1: Remove unwrap/expect/panic from production code

### Task 1: wire/protocol/event.rs

**Files:**
- Modify: `src/wire/protocol/event.rs`

- [ ] **Step 1: Replace 35 `unwrap()` calls with `?` propagation or `match`**

  Pattern: `some_option.unwrap()` → `some_option.ok_or_else(|| anyhow!("..."))?` or `if let Some(v) = some_option { ... } else { bail!(...) }`

  For serde/parse operations: use `.context("...")?`

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test wire::protocol::event
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS, 0 warnings

- [ ] **Step 3: Commit**

  ```bash
  git add src/wire/protocol/event.rs
  git commit -m "refactor(wire/protocol/event): remove unwrap from production code"
  ```

---

### Task 2: wire/protocol (content, control, jsonrpc, request, prompt, redact)

**Files:**
- Modify: `src/wire/protocol/content.rs` (14 unwrap)
- Modify: `src/wire/protocol/control.rs` (11 unwrap)
- Modify: `src/wire/protocol/jsonrpc.rs` (10 unwrap)
- Modify: `src/wire/protocol/request.rs` (9 unwrap)
- Modify: `src/wire/protocol/prompt.rs` (5 unwrap)
- Modify: `src/wire/protocol/redact.rs` (1 unwrap, 1 expect)

- [ ] **Step 1: Replace all unwrap/expect in the 6 files**

  For `redact.rs` expect: `.context("compile redact pattern")?`

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test wire::protocol
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(wire/protocol): remove unwrap/expect from content, control, jsonrpc, request, prompt, redact"
  ```

---

### Task 3: kimi_native (rollback, manifest, role_packs)

**Files:**
- Modify: `src/kimi_native/rollback.rs` (34 unwrap)
- Modify: `src/kimi_native/manifest.rs` (31 unwrap)
- Modify: `src/kimi_native/role_packs.rs` (2 unwrap)

- [ ] **Step 1: Replace all unwrap in the 3 files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test kimi_native
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(kimi_native): remove unwrap from rollback, manifest, role_packs"
  ```

---

### Task 4: cli (cleanup, team/proof, goal/validate)

**Files:**
- Modify: `src/cli/cleanup.rs` (20 unwrap)
- Modify: `src/cli/team/proof.rs` (19 unwrap)
- Modify: `src/cli/goal/validate.rs` (2 unwrap)

- [ ] **Step 1: Replace all unwrap in the 3 files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test cli
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(cli): remove unwrap from cleanup, team/proof, goal/validate"
  ```

---

### Task 5: runtime/goal/dispatch.rs

**Files:**
- Modify: `src/runtime/goal/dispatch.rs` (18 unwrap, 4 expect)

- [ ] **Step 1: Replace 18 unwrap and 4 expect with `?`/`.context()`**

  For expects like `.expect("task_proposed event")` → `.context("task_proposed event must be present")?`

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test runtime::goal::dispatch
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git add src/runtime/goal/dispatch.rs
  git commit -m "refactor(runtime/goal/dispatch): remove unwrap/expect from production code"
  ```

---

### Task 6: runtime/worker.rs + runtime/ask.rs

**Files:**
- Modify: `src/runtime/worker.rs` (17 unwrap)
- Modify: `src/runtime/ask.rs` (12 unwrap)

- [ ] **Step 1: Replace all unwrap in both files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test runtime::worker runtime::ask
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(runtime): remove unwrap from worker and ask"
  ```

---

### Task 7: runtime/shell.rs + runtime/events/writer.rs

**Files:**
- Modify: `src/runtime/shell.rs` (13 unwrap, 1 expect)
- Modify: `src/runtime/events/writer.rs` (6 unwrap, 1 expect)

- [ ] **Step 1: Replace unwrap/expect in both files**

  `shell.rs` expect: `.context("shell_escape must succeed for safe text")?`
  `writer.rs` expect: `.context("each line must be intact JSON")?`

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test runtime::shell runtime::events::writer
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(runtime): remove unwrap/expect from shell and events/writer"
  ```

---

### Task 8: runtime/goal (verifier, state, budget/usage)

**Files:**
- Modify: `src/runtime/goal/verifier.rs` (13 unwrap)
- Modify: `src/runtime/goal/state.rs` (9 unwrap)
- Modify: `src/runtime/goal/budget/usage.rs` (4 unwrap)

- [ ] **Step 1: Replace all unwrap in the 3 files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test runtime::goal::verifier runtime::goal::state runtime::goal::budget
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(runtime/goal): remove unwrap from verifier, state, budget/usage"
  ```

---

### Task 9: runtime/scheduler (claim, manifest, decompose, task)

**Files:**
- Modify: `src/runtime/scheduler/claim.rs` (11 unwrap)
- Modify: `src/runtime/scheduler/manifest.rs` (7 unwrap)
- Modify: `src/runtime/scheduler/decompose.rs` (2 unwrap)
- Modify: `src/runtime/scheduler/task.rs` (1 unwrap)

- [ ] **Step 1: Replace all unwrap in the 4 files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test runtime::scheduler
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(runtime/scheduler): remove unwrap from claim, manifest, decompose, task"
  ```

---

### Task 10: runtime/infrastructure (migrate, watchdog, retry, metrics, config, atomic, state, sanitize)

**Files:**
- Modify: `src/runtime/migrate.rs` (11 unwrap)
- Modify: `src/runtime/watchdog.rs` (8 unwrap)
- Modify: `src/runtime/retry.rs` (7 unwrap)
- Modify: `src/runtime/metrics.rs` (7 unwrap)
- Modify: `src/runtime/config.rs` (7 unwrap)
- Modify: `src/runtime/atomic.rs` (7 unwrap)
- Modify: `src/runtime/state.rs` (3 unwrap)
- Modify: `src/runtime/sanitize.rs` (4 unwrap)

- [ ] **Step 1: Replace all unwrap in the 8 files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test runtime::migrate runtime::watchdog runtime::retry runtime::metrics runtime::config runtime::atomic runtime::state runtime::sanitize
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(runtime): remove unwrap from migrate, watchdog, retry, metrics, config, atomic, state, sanitize"
  ```

---

### Task 11: vis (event_stream, hud_tui)

**Files:**
- Modify: `src/vis/event_stream.rs` (7 unwrap)
- Modify: `src/vis/hud_tui.rs` (6 unwrap)

- [ ] **Step 1: Replace all unwrap in both files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test vis::event_stream vis::hud_tui
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor(vis): remove unwrap from event_stream and hud_tui"
  ```

---

### Task 12: agents/parser + skills (parser, discovery) + runtime/wire_worker/loop_impl

**Files:**
- Modify: `src/agents/parser.rs` (2 unwrap)
- Modify: `src/skills/parser.rs` (1 unwrap)
- Modify: `src/skills/discovery.rs` (1 unwrap)
- Modify: `src/runtime/wire_worker/loop_impl.rs` (1 unwrap)

- [ ] **Step 1: Replace all unwrap in the 4 files**

- [ ] **Step 2: Run tests and clippy**

  ```bash
  cargo test agents::parser skills::parser skills::discovery runtime::wire_worker
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit -am "refactor: remove unwrap from agents/parser, skills, wire_worker/loop_impl"
  ```

---

### Task 13: Final Phase 1 verification

- [ ] **Step 1: Verify zero unwrap/expect/panic in production code**

  ```bash
  grep -rn "\.unwrap()" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "/tests.rs:" | grep -v "test_helpers" | grep -v "/tests/"
  ```
  Expected: **empty output**

  ```bash
  grep -rn "\.expect(" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "/tests.rs:" | grep -v "test_helpers" | grep -v "/tests/"
  ```
  Expected: **empty output**

  ```bash
  grep -rn "panic!(" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "/tests.rs:" | grep -v "test_helpers" | grep -v "/tests/"
  ```
  Expected: **empty output**

- [ ] **Step 2: Full test suite**

  ```bash
  cargo test
  cargo clippy --all-targets --all-features -- -D warnings
  cargo doc --no-deps
  ```
  Expected: ALL PASS

- [ ] **Step 3: Commit**

  ```bash
  git commit --allow-empty -m "refactor: verify zero unwrap/expect/panic in production code"
  ```

---

## Phase 2: Split files >400 lines into directory modules

**Rule for each split:**
1. Create `foo/mod.rs` + focused submodules
2. Move code without logic changes
3. Preserve public API via `pub use` re-exports in `mod.rs`
4. One commit per file

### Task 14: Split src/runtime/goal/dispatch.rs (800 lines)

**Files:**
- Create: `src/runtime/goal/dispatch/mod.rs`
- Create: `src/runtime/goal/dispatch/types.rs` (data structures)
- Create: `src/runtime/goal/dispatch/engine.rs` (core dispatch logic)
- Create: `src/runtime/goal/dispatch/handlers.rs` (event handlers)
- Delete: `src/runtime/goal/dispatch.rs`

- [ ] **Step 1: Split code into submodules by responsibility**
- [ ] **Step 2: Add `pub use` re-exports in `mod.rs`**
- [ ] **Step 3: Verify compilation**

  ```bash
  cargo check
  cargo test runtime::goal::dispatch
  cargo clippy --all-targets --all-features -- -D warnings
  ```
  Expected: PASS

- [ ] **Step 4: Commit**

  ```bash
  git add src/runtime/goal/dispatch/
  git rm src/runtime/goal/dispatch.rs
  git commit -m "refactor(runtime/goal/dispatch): split 800-line file into submodules per SRP"
  ```

---

### Task 15: Split src/runtime/autopilot/engine.rs (672 lines)

**Files:**
- Create: `src/runtime/autopilot/engine/mod.rs`
- Create: `src/runtime/autopilot/engine/state.rs` (state management)
- Create: `src/runtime/autopilot/engine/loop.rs` (main loop)
- Delete: `src/runtime/autopilot/engine.rs`

- [ ] **Step 1-4:** Same pattern as Task 14
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/autopilot/engine): split 672-line file into submodules"
  ```

---

### Task 16: Split src/kimi_native/manifest.rs (649 lines)

**Files:**
- Create: `src/kimi_native/manifest/mod.rs`
- Create: `src/kimi_native/manifest/parser.rs`
- Create: `src/kimi_native/manifest/validation.rs`
- Delete: `src/kimi_native/manifest.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(kimi_native/manifest): split 649-line file into submodules"
  ```

---

### Task 17: Split src/cli/app.rs (635 lines)

**Files:**
- Create: `src/cli/app/mod.rs`
- Create: `src/cli/app/commands.rs`
- Create: `src/cli/app/config.rs`
- Delete: `src/cli/app.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(cli/app): split 635-line file into submodules"
  ```

---

### Task 18: Split src/runtime/goal/verifier.rs (598 lines)

**Files:**
- Create: `src/runtime/goal/verifier/mod.rs`
- Create: `src/runtime/goal/verifier/checks.rs`
- Create: `src/runtime/goal/verifier/report.rs`
- Delete: `src/runtime/goal/verifier.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/goal/verifier): split 598-line file into submodules"
  ```

---

### Task 19: Split src/vis/hud.rs (555 lines)

**Files:**
- Create: `src/vis/hud/mod.rs`
- Create: `src/vis/hud/render.rs`
- Create: `src/vis/hud/state.rs`
- Delete: `src/vis/hud.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(vis/hud): split 555-line file into submodules"
  ```

---

### Task 20: Split src/runtime/goal/state.rs (542 lines)

**Files:**
- Create: `src/runtime/goal/state/mod.rs`
- Create: `src/runtime/goal/state/core.rs`
- Create: `src/runtime/goal/state/transitions.rs`
- Delete: `src/runtime/goal/state.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/goal/state): split 542-line file into submodules"
  ```

---

### Task 21: Split src/vis/server.rs (520 lines)

**Files:**
- Create: `src/vis/server/mod.rs`
- Create: `src/vis/server/routes.rs`
- Create: `src/vis/server/handlers.rs`
- Delete: `src/vis/server.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(vis/server): split 520-line file into submodules"
  ```

---

### Task 22: Split src/runtime/ralph.rs (517 lines)

**Files:**
- Create: `src/runtime/ralph/mod.rs`
- Create: `src/runtime/ralph/engine.rs`
- Create: `src/runtime/ralph/feedback.rs`
- Delete: `src/runtime/ralph.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/ralph): split 517-line file into submodules"
  ```

---

### Task 23: Split src/kimi_native/diagnostics.rs (468 lines)

**Files:**
- Create: `src/kimi_native/diagnostics/mod.rs`
- Create: `src/kimi_native/diagnostics/parser.rs`
- Create: `src/kimi_native/diagnostics/render.rs`
- Delete: `src/kimi_native/diagnostics.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(kimi_native/diagnostics): split 468-line file into submodules"
  ```

---

### Task 24: Split src/runtime/proof/generator.rs (454 lines)

**Files:**
- Create: `src/runtime/proof/generator/mod.rs`
- Create: `src/runtime/proof/generator/core.rs`
- Create: `src/runtime/proof/generator/formatters.rs`
- Delete: `src/runtime/proof/generator.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/proof/generator): split 454-line file into submodules"
  ```

---

### Task 25: Split src/vis/hud_tui.rs (453 lines)

**Files:**
- Create: `src/vis/hud_tui/mod.rs`
- Create: `src/vis/hud_tui/render.rs`
- Create: `src/vis/hud_tui/events.rs`
- Delete: `src/vis/hud_tui.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(vis/hud_tui): split 453-line file into submodules"
  ```

---

### Task 26: Split src/wire/protocol/event.rs (452 lines)

**Files:**
- Create: `src/wire/protocol/event/mod.rs`
- Create: `src/wire/protocol/event/types.rs`
- Create: `src/wire/protocol/event/serialization.rs`
- Delete: `src/wire/protocol/event.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(wire/protocol/event): split 452-line file into submodules"
  ```

---

### Task 27: Split src/runtime/ask.rs (419 lines)

**Files:**
- Create: `src/runtime/ask/mod.rs`
- Create: `src/runtime/ask/prompt.rs`
- Create: `src/runtime/ask/response.rs`
- Delete: `src/runtime/ask.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/ask): split 419-line file into submodules"
  ```

---

### Task 28: Split src/runtime/scheduler/claim.rs (413 lines)

**Files:**
- Create: `src/runtime/scheduler/claim/mod.rs`
- Create: `src/runtime/scheduler/claim/logic.rs`
- Create: `src/runtime/scheduler/claim/validation.rs`
- Delete: `src/runtime/scheduler/claim.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/scheduler/claim): split 413-line file into submodules"
  ```

---

### Task 29: Split src/runtime/scheduler/runner/tests.rs (412 lines)

**Files:**
- Create: `src/runtime/scheduler/runner/tests/mod.rs`
- Create: `src/runtime/scheduler/runner/tests/helpers.rs`
- Create: `src/runtime/scheduler/runner/tests/cases.rs`
- Delete: `src/runtime/scheduler/runner/tests.rs`

- [ ] **Step 1-4:** Same pattern
- [ ] **Step 5: Commit**

  ```bash
  git commit -m "refactor(runtime/scheduler/runner/tests): split 412-line test file into submodules"
  ```

---

## Phase 3: Final verification

### Task 30: Full project verification

- [ ] **Step 1: Verify no files >400 lines remain**

  ```bash
  find src -name "*.rs" -exec wc -l {} + | awk '$1 > 400 {print $1, $2}'
  ```
  Expected: **empty output**

- [ ] **Step 2: Verify zero unwrap/expect/panic in production**

  ```bash
  grep -rn "\.unwrap()" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "/tests.rs:" | grep -v "test_helpers" | grep -v "/tests/"
  grep -rn "\.expect(" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "/tests.rs:" | grep -v "test_helpers" | grep -v "/tests/"
  grep -rn "panic!(" src/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "/tests.rs:" | grep -v "test_helpers" | grep -v "/tests/"
  ```
  Expected: all **empty**

- [ ] **Step 3: Full test suite + clippy + docs**

  ```bash
  cargo test
  cargo clippy --all-targets --all-features -- -D warnings
  cargo doc --no-deps
  ```
  Expected: ALL PASS

- [ ] **Step 4: Commit**

  ```bash
  git commit --allow-empty -m "refactor: final verification — all quality gates pass"
  ```

---

## Self-Review

**Spec coverage:** Each section of the design spec maps to tasks:
- Phase 1 spec → Tasks 1–13
- Phase 2 spec → Tasks 14–29
- Testing strategy → Gates in every task + Task 30
- Commit strategy → Reflected in every task

**Placeholder scan:** No TBD/TODO/"implement later" found. All steps have exact file paths, commands, and expected outputs.

**Type consistency:** Task ordering respects dependencies (Phase 1 before Phase 2). No signature references before definition.
