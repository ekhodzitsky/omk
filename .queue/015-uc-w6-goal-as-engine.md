---
id: 015
title: UNIFIED_CHAT W6 — goal runtime as child-goal backend
status: todo
branch: feat/goal-as-engine
worktree: .worktrees/unified-chat-W6-goal-bridge
blocked_by: []
merge_after: []
size: large
batch: unified-chat-wave-1
pr: null
notes: Spec §13 W6 + §6.4 + §10. CO-1 critical — all additions go into NEW src/runtime/goal/chat_api/ subdir. Only ONE line in existing src/runtime/goal/mod.rs (pub mod chat_api;). Implements D4 wire pool (3 + spillover).
---

# Prompt

Ты работаешь в worktree `.worktrees/unified-chat-W6-goal-bridge` на ветке feat/goal-as-engine. Workstream: W6 — Goal-mode as backend.

Адаптер существующего src/runtime/goal/ для вызова из chat-сессии как child goal. Стримит goal-события на engine bus.

==================================================================
АБСОЛЮТНЫЕ ЗАПРЕТЫ
==================================================================

§12 ANTI-GOALS — все 10. Особенно:
8. No deprecation of `omk goal run` — headless mode остаётся. Твоя задача — ДОБАВИТЬ новый entry point, не заменить.
9. No magic without proof — child goal производит proof.json через существующий runtime/goal/.

§14 COORDINATION + CO-1 (КРИТИЧНО):

ТЫ ПРАКТИЧЕСКИ НЕ ТРОГАЕШЬ src/runtime/goal/. Все добавления в НОВОЙ субдиректории:
    src/runtime/goal/chat_api/  (новый — твоя единственная зона)

Исключение — ОДНА строка в src/runtime/goal/mod.rs:
    pub mod chat_api;
+ опционально re-export:
    pub use chat_api::{create_child, ChildGoalHandle};

Этой одной строки достаточно. Никаких других правок в существующих файлах src/runtime/goal/. Иначе ребейз с WS-01 (worktree/conflict), WS-05 (delivery/github_api), WS-09 (lifecycle/cleanup) превратится в ад.

Также:
    src/runtime/conversation/goal_bridge.rs — НОВЫЙ файл, ОДИН.
    (W3 владеет src/runtime/conversation/, но goal_bridge.rs предоставляешь ТЫ.)

Не трогаешь: Cargo.toml, src/lib.rs, src/main.rs, docs/UNIFIED_CHAT*.md, любые другие файлы внутри src/runtime/goal/ (кроме одной строки в mod.rs), src/cli/, src/vis/.

==================================================================
ЗАДАЧА
==================================================================

1. PUBLIC API в src/runtime/goal/chat_api/mod.rs:

        pub struct CreateChildRequest {
            pub session_id: String,
            pub parent_conv_id: String,
            pub prompt: String,
            pub config: ChildGoalConfig,
        }
        pub struct ChildGoalConfig {
            pub merge_policy: GoalMergePolicy,
            pub enforce_protection: bool,
            pub wire_pool_size: u32,                   // D4: default 3
            pub max_budget_usd: Option<f32>,
        }
        pub struct ChildGoalHandle {
            pub goal_id: String,
            pub session_id: String,
            pub created_at: DateTime<Utc>,
        }

        pub async fn create_child(req: CreateChildRequest) -> Result<ChildGoalHandle>;
        pub async fn subscribe(goal_id: &str)
            -> Result<broadcast::Receiver<ChildGoalEvent>>;

        pub enum ChildGoalEvent {
            Created { goal_id: String, plan: Vec<String> },
            PlanUpdated { revision: u32, nodes: Vec<PlanNode> },
            WorkerStarted { worker_id: String, task: String },
            WorkerProgress { worker_id: String, msg: String },
            WorkerCompleted { worker_id: String, files: u32, ok: bool },
            GateTransition { gate: String, from: String, to: String },
            SliceOpened { slice_id: String, worktree: PathBuf, pr_url: Option<String> },
            ProofReady { path: PathBuf },
            Failed { reason: String },
            Cancelled,
        }

        pub async fn pause(goal_id: &str) -> Result<()>;
        pub async fn resume(goal_id: &str) -> Result<()>;
        pub async fn cancel(goal_id: &str) -> Result<()>;
        pub async fn inject_hint(goal_id: &str, text: &str) -> Result<()>;

2. BRIDGE в src/runtime/conversation/goal_bridge.rs:
   Адаптер ChildGoalEvent → EngineEvent (W4 enum).
        pub struct GoalBridge { ... }
        impl GoalBridge {
            pub fn new(engine_bus: EventBus) -> Self;
            pub async fn attach(&self, child: ChildGoalHandle)
                -> tokio::task::JoinHandle<()>;
                // forwards events from chat_api::subscribe(child.goal_id) onto engine_bus
        }

3. WIRE POOL (D4):
   src/runtime/goal/chat_api/wire_pool.rs:
        pub struct WirePool {
            size: usize,
            idle: VecDeque<PooledWorker>,
            in_use: HashSet<String>,
            idle_ttl: Duration,
        }
        impl WirePool {
            pub fn new(size: usize) -> Self;
            pub async fn acquire(&mut self) -> Result<PooledWorker>;
            pub async fn release(&mut self, w: PooledWorker);
            // idle entries evicted after 5min via background task
        }
        pub struct PooledWorker { /* wraps existing wire client */ }
   acquire() при пустом pool и size limit — spawn fresh (не блокировать). D4 spec.

4. ИСПОЛЬЗОВАНИЕ СУЩЕСТВУЮЩЕГО runtime/goal/:
   Не дублируй goal-runtime логику. create_child(req):
   a) вызывает существующий goal::lifecycle::start (см. src/runtime/goal/lifecycle/start.rs). Конструируешь GoalRunRequest из CreateChildRequest.
   b) получает goal_id.
   c) подписывается на existing goal events (или events.jsonl reader из src/runtime/events/reader.rs).
   d) пере-эмитит как ChildGoalEvent через свой broadcast::Sender.

   Если существующий goal-runtime НЕ ПУБЛИКУЕТ события для consumption внешними подписчиками (только пишет events.jsonl) — реализуй reader-based adapter: tail events.jsonl, парсь, эмитти ChildGoalEvent. НЕ ДОБАВЛЯЙ broadcast::Sender в существующие goal-runtime файлы. Это табу CO-1.

5. SLASH COMMAND BACKEND (pub async fn в chat_api::commands):
   /show proof <goal_id>      → return path к proof.json
   /show goals <session>      → list child goals
   /show plan <goal_id>       → return current plan markdown
   /goal show <id>            → detailed status
   /replay <goal_id>          → return path к events.jsonl
   /approve <goal_id>         → call existing approve API
   /reject <goal_id> [reason] → call existing reject API

6. REPLAY:
   `omk goal replay <goal_id>` ПРОДОЛЖАЕТ работать standalone. Не ломай. /replay в chat — стримит EngineEvents из replay-источника инлайн в conversation log.

==================================================================
РАЗВЕДКА
==================================================================

1. src/runtime/goal/lifecycle/start.rs — есть ли pub fn для запуска извне?
2. src/runtime/goal/types.rs — GoalRunRequest / GoalConfig?
3. src/runtime/goal/replay.rs — replay-API.
4. src/runtime/events/reader.rs — JSONL reader.
5. src/wire/client.rs + src/runtime/wire_worker/ — как создаётся wire worker.
6. src/runtime/goal/delivery/pr_client.rs — для approve/reject.

==================================================================
ТЕСТЫ (минимум 8)
==================================================================

tests/goal_chat_api_test.rs:
- test_create_child_returns_handle_with_goal_id
- test_subscribe_receives_created_event
- test_subscribe_propagates_plan_updates
- test_subscribe_propagates_proof_ready_with_correct_path
- test_pause_resume_round_trip
- test_cancel_emits_cancelled_event
- test_wire_pool_reuses_idle_worker
- test_wire_pool_spills_to_fresh_when_size_exceeded
- test_replay_existing_goal_via_chat_api
- test_existing_omk_goal_run_headless_still_works (smoke: cargo run --bin omk goal run --help)

Используй InMemoryWireClient (src/wire/) + mock goal-runtime если нужно. Не запускай реальные goal'ы в интеграционных тестах W6.

==================================================================
СКЕЛЕТ
==================================================================

src/runtime/goal/chat_api/mod.rs:
    pub mod commands;
    pub mod handle;
    pub mod events;
    pub mod wire_pool;
    pub mod source;        // events.jsonl tail adapter
    mod adapter;           // existing goal runtime → ChildGoalEvent

    pub use handle::*;
    pub use events::ChildGoalEvent;

    pub async fn create_child(req: CreateChildRequest) -> Result<ChildGoalHandle>;
    pub async fn subscribe(goal_id: &str) -> Result<broadcast::Receiver<ChildGoalEvent>>;
    pub async fn pause(goal_id: &str) -> Result<()>;
    pub async fn resume(goal_id: &str) -> Result<()>;
    pub async fn cancel(goal_id: &str) -> Result<()>;
    pub async fn inject_hint(goal_id: &str, text: &str) -> Result<()>;

src/runtime/conversation/goal_bridge.rs:
    pub struct GoalBridge { ... }

==================================================================
SUCCESS CRITERIA
==================================================================

- cargo build --workspace зелено
- cargo test --test goal_chat_api_test зелено ≥8
- cargo test --workspace — ничего смежного не сломано (включая существующие goal tests)
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status:
    new file:   src/runtime/goal/chat_api/<files>
    new file:   src/runtime/conversation/goal_bridge.rs
    modified:   src/runtime/goal/mod.rs    (РОВНО одна строка pub mod chat_api;)
    new file:   tests/goal_chat_api_test.rs
- PR title: "feat(goal): chat_api child-goal bridge with wire pool (W6)"
- PR body содержит:
  * подтверждение что existing `omk goal run` headless не сломан
  * запрос на coordination для регистрации goal_bridge.rs в src/runtime/conversation/mod.rs (это W3's tree)

==================================================================
СТОП-ТРИГГЕРЫ
==================================================================

- Существующий goal-runtime не имеет публичного entry point для запуска извне → STOP. Возможно нужен мини-рефактор, но это ОТДЕЛЬНЫЙ PR через orchestrator.
- broadcast::Sender нужен в существующих goal-файлах → STOP, не правь. Используй events.jsonl tail adapter.
- Wire pool требует изменений в src/wire/client.rs → STOP, обсуди; возможно нужно обернуть, не модифицировать.
- Cargo.toml требует новых deps → запроси в PR body.

НЕ ПЕРЕСМАТРИВАЙ §6, §10, §13, §14.

Начинай с разведки.
