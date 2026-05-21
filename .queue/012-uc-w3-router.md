---
id: 012
title: UNIFIED_CHAT W3 — router and escalation bridge
status: todo
branch: feat/router
worktree: .worktrees/unified-chat-W3-router
blocked_by: []
merge_after: [011, 015]
size: large
batch: unified-chat-wave-1
pr: null
notes: Spec §13 W3 + §6. Work parallel-safe (uses mocks for W2/W6); merge sequential after W2 and W6. Implements all D1–D8.
---

# Prompt

Ты работаешь в worktree `.worktrees/unified-chat-W3-router` на ветке feat/router. Workstream: W3 — Router and escalation bridge.

Клеящий слой между классификатором (W2), shell (W1), engine pane (W4) и goal-runtime (W6). Соблюдает D1–D8 из docs/UNIFIED_CHAT_DECISIONS.md.

ВАЖНО: spec §14.8 — W3 мерджится ПОСЛЕ W2 и W6. Ты пишешь код против интерфейсов, которые ещё не финализированы. Поэтому: определи МИНИМАЛЬНЫЕ trait для интеграции, impl MockClassifier / MockGoalBridge в тестах. Реальная интеграция — coordination day orchestrator'а.

==================================================================
АБСОЛЮТНЫЕ ЗАПРЕТЫ
==================================================================

§12 ANTI-GOALS — все 10. Особенно:
1. No silent side effects — каждое side-effectful escalation эмитит disclosure line ДО side effect'а (§6.3). Hard rule.
7. No stub commands — все slash из §8.2 либо работают через W3, либо отсутствуют в W3.
9. No magic without proof — large = создание child goal через W6 API.

§14 COORDINATION:
- ТЫ МОЖЕШЬ ИЗМЕНЯТЬ ТОЛЬКО:
    src/runtime/conversation/   (новый)
    src/runtime/escalation/     (новый)
    tests/router_integration_test.rs
- ТЫ НЕ ТРОГАЕШЬ: src/runtime/classifier/ (W2), src/runtime/goal/ (W6+audit), src/cli/chat/ (W1), src/vis/ (W1/W4), Cargo.toml, src/lib.rs, src/main.rs, src/runtime/mod.rs, docs/UNIFIED_CHAT*.md.

==================================================================
ЗАДАЧА
==================================================================

1. ROUTER:
        pub struct Router {
            classifier: Box<dyn ClassifierBackend>,
            wire_pool: WirePool,
            goal_bridge: Box<dyn GoalBridge>,
            llm_direct: Box<dyn LlmDirectBackend>,
            config: RouterConfig,
            state: Arc<Mutex<RouterState>>,
            event_bus: EventBus,
        }
        pub struct RouterConfig {
            pub medium_goal_cap: u32,                // D1: default 3
            pub cost_cap_usd_soft: Option<f32>,
            pub cost_cap_usd_hard: Option<f32>,
            pub first_prompt_threshold: f32,         // D3: 0.85
            pub wire_pool_size: u32,                 // D4: 3
        }
        pub async fn dispatch(&self, prompt: &str, session: &SessionCtx) -> RouteOutcome;

2. МАТРИЦА ДИСПАТЧА (§6.1):
   Trivial → llm_direct.call → stream → conversation log. No preflight, no pane.
   Small   → wire_pool.spawn(SmallEdit) → diff → preflight ONLY if >5 files OR protected → [A]/[R]/[D].
   Medium  → tiny planner (3-7 steps), sequential workers, plan checkbox UI events для W4.
   Large   → ALWAYS preflight. On Enter: goal_bridge.create_child(session_id, parent_conv_id, prompt) → goal_id → subscribe → ретрансляция в event_bus.

3. PREFLIGHT DIALOG (§6.2):
        pub struct Preflight {
            pub kind: PreflightKind,
            pub headline: String,
            pub timeout_ms: u32,  // 60_000
        }
        pub enum PreflightKind { LargeEscalation, MediumLowConfidence, SmallOverProtected }

   W3 эмитит Event::PreflightRequest(Preflight) в шину. W1 рисует и ловит [Enter]/[E]/[Q]/[Esc], эмитит Event::PreflightResponse(PreflightAction).

   На Q (D6): Large→Medium downgrade, dispatch immediately as medium. Medium→Small. Small — no-op log "already lowest". Без re-classify.

4. DISCLOSURE LINES (§6.3) — НЕЛЬЗЯ ПРОПУСКАТЬ:
   Trivial: no disclosure.
   Small:   "→ small edit: single-worker rename"
   Medium:  "→ medium task: <N>-step plan, sequential workers"
   Large:   "→ large feature: launching goal-mode (slice PR will be created)"
   Эмитится в conversation log ДО любого worker spawn.

5. CHILD GOAL LIFECYCLE (§6.4):
   Concurrency (D1, §9.6): 0..1 large, 0..medium_goal_cap medium (default 3), 0..3 small.
   Попытка escalate в large при активном large — preflight + "(another large goal is running; this one will queue)" → user confirm → queue, не start.

6. USER OVERRIDES API:
        pub async fn dispatch_with_intent_override(
            prompt: &str, override_intent: Intent, session: &SessionCtx
        ) -> RouteOutcome;
   W3 не парсит slash — это W5. /quick, /escalate, /explain, /classify дёргают W3 API.

7. COST CAP (D2):
   Перед dispatch:
   - cost_usd >= cost_cap_usd_hard (if Some): RouteOutcome::Refused + "→ refused: hard cost cap exceeded. /cost for details, raise cap in .omk/config.toml"
   - cost_usd >= cost_cap_usd_soft (if Some) AND not warned this session: один-раз warning "⚠ cost crossed soft cap ($X.XX). /cost to inspect.", session.cost_soft_warned = true.

8. FIRST-PROMPT THRESHOLD (D3):
   SessionCtx::is_first_prompt — bool, true пока ни один dispatch не завершился ok. Когда true: effective_preflight_threshold = config.first_prompt_threshold (0.85). Иначе 0.65.

9. EVENT BUS:
   src/runtime/conversation/bus.rs:
        pub enum BusEvent {
            ClassifierDecided { intent, confidence, latency_ms, reasoning },
            RouterEscalating { intent, target_mode, preflight: bool },
            WorkerStarted { worker_id, kind, task },
            WorkerProgress { worker_id, percent: Option<f32>, message: Option<String> },
            WorkerCompleted { worker_id, files_touched, ok: bool },
            GoalCreated { goal_id, parent_session, plan },
            GoalPlanUpdated { goal_id, revision, nodes },
            GoalGateTransition { goal_id, gate, from, to },
            GoalProofReady { goal_id, path },
            CostDelta { source, tokens_in, tokens_out, usd },
            PreflightRequest(Preflight),
            PreflightResponse(PreflightAction),
        }
        pub struct EventBus { tx: tokio::sync::broadcast::Sender<BusEvent> }
        // capacity 1024, subscribe/publish

==================================================================
СКЕЛЕТ
==================================================================

src/runtime/escalation/mod.rs:
    pub mod router;
    pub mod preflight;
    pub mod wire_pool;
    pub mod planner;
    pub mod overrides;
    pub mod mocks;

src/runtime/conversation/mod.rs:
    pub mod bus;
    pub mod session;
    pub mod disclosure;
    pub mod outcome;

==================================================================
МОКИ
==================================================================

src/runtime/escalation/mocks.rs:
    pub trait ClassifierBackend: Send + Sync {
        async fn classify(&self, input: ClassifierInput) -> ClassifierOutput;
    }
    pub trait GoalBridge: Send + Sync {
        async fn create_child(&self, session_id: &str, parent_conv_id: &str,
                              prompt: &str) -> Result<String>;
        async fn subscribe(&self, goal_id: &str)
            -> Box<dyn Stream<Item = GoalEvent> + Send + Unpin>;
    }
    pub struct MockClassifier { ... }
    pub struct MockGoalBridge { ... }

==================================================================
ТЕСТЫ (минимум 10)
==================================================================

tests/router_integration_test.rs:
- test_trivial_intent_skips_preflight_and_emits_no_disclosure
- test_small_intent_emits_disclosure_then_spawns_worker
- test_medium_intent_creates_plan_with_N_steps_and_emits_events
- test_large_intent_always_preflights_before_creating_goal
- test_concurrency_cap_blocks_fourth_medium_goal
- test_first_prompt_threshold_is_stricter_than_subsequent
- test_quick_override_forces_small_intent
- test_escalate_override_forces_large_with_preflight
- test_preflight_q_downgrades_one_level
- test_soft_cost_cap_warns_once_per_session
- test_hard_cost_cap_refuses_dispatch
- test_disclosure_line_emitted_before_any_side_effect

==================================================================
SUCCESS CRITERIA
==================================================================

- cargo build --workspace зелено
- cargo test --test router_integration_test зелено (≥10)
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status: только src/runtime/conversation/**, src/runtime/escalation/**, tests/router_integration_test.rs
- PR title: "feat(router): intent dispatcher with preflight, child goals, cost cap (W3)"
- PR body: описание интерфейсов для склейки с W2 и W6, подтверждение D1–D8.

==================================================================
СТОП-ТРИГГЕРЫ
==================================================================

- W6 child-goal API не очевиден → используй mock, опиши ожидаемую сигнатуру в PR.
- W2 ClassifierBackend trait несовместим → используй mock; orchestrator склеит на coordination day.
- Concurrency tests флакают → используй tokio::test с deterministic time или mutex sync. НЕ sleep.

НЕ ПЕРЕСМАТРИВАЙ §5, §6, §9, §13, §14, §16. Решения зафиксированы в docs/UNIFIED_CHAT_DECISIONS.md (D1–D8).

Начинай с разведки.
