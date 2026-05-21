# UNIFIED CHAT — Orchestrator Decisions

> **Purpose:** Resolve the eight open questions in `docs/UNIFIED_CHAT.md` §16 before W3 (router) merges. Each decision is load-bearing for at least one Wave-1 workstream and must be honoured by the worker agents.
>
> **Status:** Final. Do not revisit without explicit user approval.
>
> **Decided:** 2026-05-21 by orchestrator.

---

## D1 — Concurrent medium goals cap

**Question (§16.1):** Is the default cap of 3 concurrent medium goals right?

**Decision:** **Keep default at 3.** Configurable via `.omk/config.toml::medium_goal_cap` (1..=8). W3 implements both the cap and the config key.

**Rationale:** Parallelism is the wedge (§1), but >3 concurrent medium goals on a shared working tree starts to fight itself (lock contention, conflicting edits, cognitive load on user). 3 covers realistic burst. Telemetry (§10.4) will tell us if users routinely hit the cap.

**Implementer:** W3.

---

## D2 — Cost cap behaviour

**Question (§16.2):** Hard stop or soft warning on cost-cap trip?

**Decision:** **Soft warning by default. Optional hard stop via opt-in config.** Two config keys:
- `cost_cap_usd_soft` — when crossed, single conversation-log warning line + cost-pane indicator. No execution change. Default: unset.
- `cost_cap_usd_hard` — when crossed, new escalations (small/medium/large) are refused with a clear inline error. Running ones finish. Default: unset.

**Rationale:** Hard auto-stop is hostile UX. User retains agency via `/cost` self-service (§8.2). Hard cap exists as opt-in for users who want it (CI loops, supervised runs).

**Implementer:** W3 (escalation refusal), W4 (cost-meter visual treatment of crossed soft cap).

---

## D3 — First-prompt threshold

**Question (§16.3):** How aggressively does the first-ever prompt of a session pre-flight?

**Decision:** **Pre-flight at confidence < 0.85 (vs normal 0.65) for the first prompt of every session, not just first-ever per user.** Threshold reverts to normal (0.65) after the first prompt successfully completes (`ok` outcome on classifier event).

**Rationale:** First-impression risk is high (§11 risk row). Friction is one Enter press. Re-arming on every session start, not just first-ever, is cheap and consistent: each new session starts with the user fresh, not necessarily comparable to the previous one.

**Implementer:** W3.

---

## D4 — Wire worker pool sizing

**Question (§16.4):** Reusable pool or fresh process per task?

**Decision:** **Pool of 3, fresh process when pool exhausted.** Pool entries idle-evict after 5 minutes. Configurable via `.omk/config.toml::wire_pool_size` (0..=8; 0 = always fresh, never pool).

**Rationale:** Pool reuse cuts ~200–800 ms startup on the common case. Spillover-to-fresh keeps burst capacity. Idle eviction caps memory.

**Implementer:** W6 (pool abstraction), W3 (configurable knob).

---

## D5 — Telemetry retention

**Question (§16.5):** How long to keep `~/.local/state/omk/telemetry.json`?

**Decision:** **30 days rolling, max override 90 days.** Configurable via `.omk/config.toml::telemetry_retain_days` (1..=90). On read, entries older than the retain window are dropped and the file is rewritten compacted.

**Rationale:** Local-only telemetry value drops fast. 30d covers two calibration cycles. Max 90 caps growth even for users who set high values. (Spec §16 suggested 90; reducing to 30 default keeps the file lean. User can opt in.)

**Implementer:** W2 (or a small helper inside the classifier subsystem that owns the file).

---

## D6 — Pre-flight `Q` behaviour for large

**Question (§16.6):** What does `Q` do exactly when intent is large?

**Decision:** **Downgrade one level and dispatch with new intent. Do NOT re-classify.** large→medium on first `Q`. medium→small on second `Q` if pre-flight surfaces again (it will if confidence is still low). `Q` on small is a no-op (already lowest non-trivial; user has overrides via `/quick` or just typing a more focused prompt).

**Rationale:** `Q` (= "keep it quick") is a user signal of disagreement with the classifier. Re-classifying adds latency without honouring the signal. Direct downgrade preserves user intent. Re-classifying is W2's job; here we just dispatch.

**Implementer:** W3.

---

## D7 — Crash resume policy

**Question (§16.7):** On chat restart after crash — auto-restore or ask?

**Decision:** **Ask with 5-second timeout that defaults to "yes, restore".** Question rendered as a single inline conversation line in the new session, identical to a pre-flight dialog visually. Keys: `Enter` accept, `Esc` decline (start fresh). Timeout = 5s wall clock, then auto-accept.

**Rationale:** Respects user agency (they may have crashed intentionally to abandon work) without blocking the common case where the user wants to continue. Matches "ship velocity" mindset (§16 hand-off note).

**Implementer:** W1 (resume UX), coordinating with W6 for goal-state recovery scan.

---

## D8 — Slash command unknown handling

**Question (§16.8):** Sentence starts with `/` but is not a valid slash command — what happens?

**Decision:** **Treat as text. Emit a one-time-per-session hint** on the first occurrence: `[that's not a command; sending as text. use /help for available commands.]`. Subsequent unknown `/` prompts dispatch as text silently.

**Rationale:** Minimal friction (no error popups, no rejection of input). One hint per session educates without nagging. If the user routinely uses `/` for non-commands (e.g. file paths), they only see the hint once per session.

**Implementer:** W5 (command parser + hint emission).

---

## Coordination notes for workers (cross-cutting)

These are not §16 decisions, but recording them here so they have a citable home.

### CO-1 — `src/runtime/goal/` collision avoidance

Several in-flight audit-track workstreams touch `src/runtime/goal/` (see `UNIFIED_CHAT_BASE.md`). **W6 must NOT add code into existing files inside `src/runtime/goal/`**, except for one line in `src/runtime/goal/mod.rs` that declares `pub mod chat_api;`. All W6 public-API code lives under `src/runtime/goal/chat_api/` (new subdir). This keeps collision surface limited to a single trivially-rebaseable line.

### CO-2 — Cargo.toml and module registration

Per §14.3, workers do NOT touch `Cargo.toml`. If they need a new dep, they request it in their PR description; orchestrator adds it via a separate "coordination PR" batched daily. Same applies to `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`. Submodule registration inside a workstream's own owned subtree is allowed.

### CO-3 — UNIFIED_CHAT.md spec file is not yet on master

At base-SHA time, `docs/UNIFIED_CHAT.md` exists as a draft in the orchestrator's context but is not committed. Worker prompts inline the relevant sections (§3 UX, §13 their workstream, §14 coordination protocol, §12 anti-goals, and this decisions file). Workers MUST NOT recreate the full spec file in their PRs.

### CO-4 — `feat/intent-classifier` does not modify `src/llm/`

The spec §13 W2 says owned modules include `src/llm/classifier_prompt.rs`. Today's `src/llm/` has its own in-flight refactor (LLM client trait, planner wire-up). To keep W2 from colliding with that subsystem, the classifier system-prompt template lives at `src/runtime/classifier/system_prompt.rs` (NOT `src/llm/classifier_prompt.rs`). W2 worker prompt reflects this redirect.

### CO-5 — Anti-goals are hard rules

§12 anti-goals are not negotiable by any worker. If a worker proposes an implementation that violates §12 (e.g. silent escalation, telemetry transmission, cloud control plane), the reviewer agent must block the PR. Examples already seen in similar projects:

- "Caching the cost meter to a remote endpoint" — violates §12.4.
- "Falling back to OpenAI when Kimi is offline" — violates §12.2.
- "Adding `omk init --web` that hosts a wizard" — violates §12.5.
- Stub slash commands that print "TODO" — violates §12.7.

---

## Sign-off

These eight decisions plus five coordination notes are the operational contract for Wave 1. Workers reference this file in their PR descriptions. Orchestrator reviews every PR for §12 anti-goal compliance and §14 coordination boundary respect.
