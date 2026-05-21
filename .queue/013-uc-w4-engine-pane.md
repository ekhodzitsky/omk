---
id: 013
title: UNIFIED_CHAT W4 — engine pane rendering
status: todo
branch: feat/engine-pane
worktree: .worktrees/unified-chat-W4-engine-pane
blocked_by: []
merge_after: [010]
size: large
batch: unified-chat-wave-1
pr: null
notes: Spec §13 W4 + §7. Snapshot-based tests, no shell needed. Defines EngineEvent enum (vis-side projection of W3's BusEvent).
---

# Prompt

Ты работаешь в worktree `.worktrees/unified-chat-W4-engine-pane` на ветке feat/engine-pane. Workstream: W4 — Engine pane.

Правая панель TUI: рендерит классификатор, план, workers, evidence-gates, cost-meter. Подписывается на event bus, рендерит секции.

==================================================================
АБСОЛЮТНЫЕ ЗАПРЕТЫ
==================================================================

§12 ANTI-GOALS — особенно:
4. No telemetry transmission — pane показывает локальные счётчики, НЕ отправляет.
5. No web UI.
7. No stub commands.

§14 COORDINATION:
- ТЫ МОЖЕШЬ ИЗМЕНЯТЬ ТОЛЬКО:
    src/vis/engine/             (новый)
    src/vis/bus.rs              (новый, один файл)
    tests/engine_pane_snapshot_test.rs
    tests/fixtures/engine_pane/**
- ТЫ НЕ ТРОГАЕШЬ: src/vis/shell/, src/vis/hud_tui/, src/vis/hud/, src/vis/server/, src/vis/event_stream.rs, Cargo.toml, src/lib.rs, src/main.rs, src/vis/mod.rs (orchestrator подключит pub mod engine; pub mod bus;).

ВАЖНО про src/vis/bus.rs:
W3 определит свой BusEvent в src/runtime/conversation/bus.rs (tokio broadcast). Это РАЗНЫЕ места. W3-bus — runtime/конкурентная шина. W4-bus — публичные event-типы, которые pane знает рендерить.

В src/vis/bus.rs определи pub enum EngineEvent — структурно как BusEvent W3, но без tokio-specific полей. W3 при публикации трансформирует в EngineEvent и шлёт subscriber-у pane'а. Lossy projection — нормально.

==================================================================
ЗАДАЧА
==================================================================

1. СОСТОЯНИЯ (§7.1):
        pub enum PaneState { Collapsed, Compact, Expanded }
   Default = Collapsed. После escalation small или выше — auto-expand в Compact. 60s idle — auto-collapse. Tab toggles Compact <-> Expanded. Shift-Tab → Collapsed.

2. СЕКЦИИ EXPANDED MODE (§7.2):
   1) Session header: id, project root short, uptime, cumulative cost
   2) Classifier: latest intent + confidence + latency + recent 5
   3) Active mode: idle | direct-llm | wire-worker | planner+workers | goal-run
   4) Plan: medium → linear checklist; large → tree, current highlighted
   5) Workers: per-worker line ● / ⧗ / ✓ + id + task + elapsed
   6) Evidence gates (large only): tests / security / integrator / custom
   7) Slice / worktree (large only): slice_id + path + PR URL
   8) Cost meter: tokens in/out current step + cumulative session
   9) Footer: hotkey hint

3. COMPACT MODE: только header + classifier + active mode + workers (1-3).

4. COLLAPSED MODE: одна строка
        [engine] o7k_a8f2 · goal-run · workers 2/3 · cost: $0.42 · Tab

5. ТЕМИЗАЦИЯ:
   - Dark/Light палитра, colour-blind safe.
   - Все цветовые сигналы дублируются символами (● ✓ ✗ ⧗).
   - Spinner: ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏

6. ПУСТЫЕ СЕКЦИИ — СКРЫВАТЬ. Не рисуй "no workers running".

7. EVENTS:
   Subscribe to broadcast::Receiver<EngineEvent>. Apply каждое к PaneModel. PaneModel — чистая структура без I/O. Рендер — pure function PaneModel → ratatui::Frame ops.

==================================================================
EngineEvent enum (src/vis/bus.rs)
==================================================================

    pub enum EngineEvent {
        ClassifierDecided { intent: Intent, confidence: f32,
                            latency_ms: u32, reasoning: String },
        RouterEscalating { intent: Intent, target_mode: ActiveMode,
                           preflight: bool },
        WorkerStarted { worker_id: String, kind: String, task: String },
        WorkerProgress { worker_id: String, percent: Option<f32>, message: Option<String> },
        WorkerCompleted { worker_id: String, files_touched: u32, ok: bool },
        GoalCreated { goal_id: String, parent_session: String, plan: Vec<String> },
        GoalPlanUpdated { goal_id: String, revision: u32, nodes: Vec<PlanNode> },
        GoalGateTransition { goal_id: String, gate: String, from: String, to: String },
        GoalProofReady { goal_id: String, path: PathBuf },
        CostDelta { source: String, tokens_in: u32, tokens_out: u32, usd: f32 },
        SessionTick,
    }

    pub enum Intent { Trivial, Small, Medium, Large }
    pub enum ActiveMode { Idle, DirectLlm, WireWorker, PlannerWorkers, GoalRun }
    pub struct PlanNode { pub id: String, pub label: String, pub status: PlanNodeStatus }
    pub enum PlanNodeStatus { Pending, Running, Done, Failed }

==================================================================
ТЕСТЫ — golden snapshots
==================================================================

tests/engine_pane_snapshot_test.rs:
1. Дано — recorded engine-events.jsonl (fixtures в tests/fixtures/engine_pane/<scenario>.jsonl).
2. Прогоняй события через PaneModel.
3. Рендер в `Vec<String>` (buffer-rendered ratatui frame, stripped escapes).
4. Сравнивай с golden snapshot tests/fixtures/engine_pane/<scenario>.snap.

Минимум 6 сценариев:
- scenario_collapsed_idle.jsonl
- scenario_compact_classifier_decided.jsonl
- scenario_expanded_small_worker_running.jsonl
- scenario_expanded_medium_plan_3of4_done.jsonl
- scenario_expanded_large_goal_running.jsonl
- scenario_cost_meter_accumulates_correctly.jsonl

==================================================================
СКЕЛЕТ
==================================================================

src/vis/engine/mod.rs:
    pub mod model;
    pub mod render;
    pub mod sections;
    pub mod state;
    pub mod theme;

src/vis/bus.rs:
    pub enum EngineEvent { ... }
    pub struct EngineSubscriber { rx: tokio::sync::broadcast::Receiver<EngineEvent> }

==================================================================
SUCCESS CRITERIA
==================================================================

- cargo build --workspace зелено
- cargo test --test engine_pane_snapshot_test зелено (≥6 сценариев)
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status: только src/vis/engine/**, src/vis/bus.rs, tests/engine_pane_snapshot_test.rs, tests/fixtures/engine_pane/**
- PR title: "feat(engine): pane rendering with snapshot tests (W4)"
- PR body: описание EngineEvent contract для W3, запрос на pub mod engine; pub mod bus; в src/vis/mod.rs.

==================================================================
СТОП-ТРИГГЕРЫ
==================================================================

- ratatui отсутствует в Cargo.toml → STOP, запроси.
- src/vis/hud_tui использует ratatui другой major-версии → используй ту же major, не апдейть.
- Snapshot тесты flaky из-за timestamps → mock-clock в тестах.

НЕ ПЕРЕСМАТРИВАЙ §3, §7, §13, §14.

Начинай с разведки.
