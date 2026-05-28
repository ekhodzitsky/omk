# Критичные точки рефакторинга oh-my-kimi

**Дата:** 2026-05-27  
**Scope:** `src/**/*.rs`  
**Методология:** Архитектурный анализ, дедупликация, dead code, когнитивная сложность

---

## P0 — Архитектурный deadlock (блокируют развитие)

### P0.1 Циклическая зависимость `cli ↔ vis`

**Цепочка:**
- `src/cli/chat/events_adapter.rs:6` → `use crate::vis::bus::{...}`
- `src/vis/shell/conversation_view.rs:1` → `use crate::cli::chat::persistence::Message;`
- `src/vis/shell/engine_placeholder.rs:1` → `use crate::cli::chat::app::PaneState;`
- `src/vis/shell/input_view.rs:1` → `use crate::cli::chat::input::InputMode;`
- `src/vis/shell/keymap.rs:1` → `use crate::cli::chat::commands::parser::Command;`
- `src/vis/shell/layout.rs:1` → `use crate::cli::chat::app::PaneState;`

**Проблема:** `cli` — оркестратор, `vis` — визуализация. `vis` не должен знать о `cli`. Цикл мешает компилировать модули независимо и нарушает AGENTS.md §Agent Module Architecture.

**Рефакторинг:** Ввести `vis::shell::types` с trait-контрактами (`ChatMessage`, `PaneState`, `InputMode`). `cli::chat` реализует эти traits. Удалить все `use crate::cli::chat::*` из `vis/shell/`.

---

### P0.2 `runtime/` импортирует `cost/` и `notifications/`

**Файлы:**
| Файл | Строки | Что импортирует |
|---|---|---|
| `src/runtime/goal/budget.rs` | 26–29 | `crate::cost::estimator`, `file_sink`, `tracker`, `types` |
| `src/runtime/session.rs` | 9, 13, 16 | `crate::cost::*`, `crate::notifications::*` |

**Проблема:** AGENTS.md §2: «`runtime/` must not import `cost/`, `notifications/`, `vis/`, or `cli/`». `budget.rs` инстанцирует `JsonFileCostSink` и `CostTracker`. `session.rs` отправляет webhook-уведомления.

**Рефакторинг:**
- `budget.rs`: принимать `&dyn CostSink` из `cli/` через DI.
- `session.rs`: переместить в `src/cli/session.rs`. Удалить `pub mod session` из `runtime/mod.rs:19`.

---

### P0.3 Dual state persistence (JSON ↔ SQLite)

**Файл:** `src/runtime/goal/state/store.rs`

| Операция | Пишет в | Читает из |
|---|---|---|
| `save()` | SQLite + JSON | — |
| `load()` | — | JSON (fallback DB) |
| `list()` | — | DB (fallback JSON scan) |

**Проблема:** Нет единого источника истины. `DbGoalStateStore` (`db_store.rs`) обёрнут в `#[allow(dead_code)]` (7 блоков), но фактически используется внутри `FileSystemGoalStateStore`. Это не "dead code", это "zombie code".

**Рефакторинг:** Выбрать SQLite как primary. JSON — read-only fallback для миграции. Удалить `#[allow(dead_code)]`. Мапперы `goal_state_to_record` (290 LOC) и `record_to_goal_state` (120 LOC) — ручное копирование 26 полей.

---

### P0.4 `lib.rs` — всё `pub mod`

**Файл:** `src/lib.rs:8–24`

Все 15 модулей — `pub mod`. AGENTS.md §Project Contract #2: «Prefer `pub(crate)`».

**Проблема:** `skills`, `agents`, `marketplace`, `notifications`, `analysis` — модули с нулевыми внешними потребителями, но экспортируются как публичное API. Это обязывает к backward compat даже для orphaned кода.

**Рефакторинг:**
- `pub(crate) mod` для `skills`, `agents`, `marketplace`, `notifications`, `analysis`.
- `#[cfg(test)] pub mod test_helpers;` вместо `#[doc(hidden)] pub mod test_helpers;`.

---

## P0 — Когнитивный коллапс (невозможно поддерживать без рефакторинга)

### P0.5 `wire_worker/task/process.rs` — `process_task` (~360 LOC, 132 пробела отступ)

**Структура:**
```
tokio::select! {
  match msg {
    Ok(WireMessage::Event(ev)) => {
      match ev.params.to_event() {
        Ok(typed) => match typed { ... 8 arms ... }
        Err(_) => match ev.params.normalized_event_type().as_str() { ... 10 arms ... }
      }
    }
    Ok(WireMessage::Request(req)) => match req.params.to_request() {
      Ok(request) => {
        if let Request::HookRequest(hook_req) = request { ... }
        if let Request::ToolCallRequest(tool_call) = request { ... }
        match &request { ... approval ... }
      }
      Err(_) => { ... }
    }
    ...
  }
}
```

**Рефакторинг:** Extract methods с `ControlFlow`:
```rust
async fn handle_event(&mut self, ev: Event) -> Result<ControlFlow>
async fn handle_request(&mut self, req: Request) -> Result<ControlFlow>
async fn handle_hook(&mut self, hook_req: HookRequest) -> Result<()>
async fn handle_tool_call(&mut self, tool_call: ToolCallRequest) -> Result<()>
```
Целевая длина `process_task`: ~80 LOC. Целевая глубина отступа: ~60 пробелов.

---

### P0.6 `goal/lifecycle/cleanup.rs` — `process_slice_delivery_and_review` (~250 LOC, 126 пробел)

**Что делает:** 6 разных concern'ов в одной функции:
1. PR delivery вызов
2. Anti-slop task spawn + event write
3. Security cleanup task spawn + event write
4. Delivery result mapping (4 ветки по 30–50 LOC)
5. Task graph mutation
6. Metadata update

**Рефакторинг:**
```rust
async fn deliver_and_review(...) -> DeliveryOutcome { ... }
async fn spawn_post_delivery_tasks(...) -> Result<()> { ... }
fn update_task_from_delivery(task_graph, slice, delivery) { ... }
```

---

### P0.7 `goal/state/db_store.rs` — ручные мапперы (410 LOC)

**Файл:** `src/runtime/goal/state/db_store.rs`

- `goal_state_to_record` — ~290 LOC (26 полей ручным копированием)
- `record_to_goal_state` — ~120 LOC
- 5 мелких парсеров: `parse_goal_status`, `parse_goal_phase`, `parse_delivery_policy`, `parse_merge_policy`

**Рефакторинг:**
- Intermediate struct с `#[derive(Serialize, Deserialize)]` + `rusqlite::named_params!`.
- Или макрос `derive_from_row!` для 26 полей.
- Вынести мапперы в `src/runtime/goal/state/mappers.rs`.

---

### P0.8 `db/repo/goal.rs` — 26 позиционных `?1..?26`

**Файл:** `src/runtime/db/repo/goal.rs`

```rust
conn.execute(
    "INSERT INTO goals (goal_id, status, phase, ... 26 columns ...) VALUES (?1, ?2, ... ?26)",
    params![goal.goal_id, goal.status, ..., goal.version],
)?;
```

Глубина отступа: 152 пробела.

**Рефакторинг:**
- Named params: `&[(":goal_id", &goal.goal_id), ...]`
- Или `#[derive(rusqlite::ToSql)]` на intermediate struct
- Или `goal.to_params()` — метод, возвращающий `rusqlite::Params`

---

## P1 — Дублирование (dedup даёт немедленную выгоду)

### P1.1 `run_kimi` — 3 копии

| Файл | Строка | Отличия |
|---|---|---|
| `ralph/runner.rs:10` | `args(["-p", prompt]).current_dir(dir)` | `warn!` при ошибке |
| `ultrawork.rs:157` | идентично | `anyhow::bail!` при ошибке |
| `autopilot/helpers.rs:31` | `arg("--print")` | без `dir` |

**Рефакторинг:**
```rust
// runtime/shell.rs или runtime/kimi.rs
pub async fn run_kimi_prompt(prompt: &str, dir: Option<&Path>, print: bool, timeout: Duration) -> Result<String>
```

---

### P1.2 `detect_changed_files` — 2 копии

| Файл | Характеристики |
|---|---|
| `gates/detect.rs:109` | async, timeout(10s), `parse_porcelain_changed_file` |
| `goal/review/architect.rs:212` | sync (`std::process::Command`), без таймаута, `parse_porcelain_path` |

**Рефакторинг:** Удалить копию в `architect.rs`. Использовать `gates::detect_changed_files(worktree).await`.

---

### P1.3 `parse_porcelain_*` — 2 копии

| Файл | Строка |
|---|---|
| `gates/detect.rs:128` | `line[3..].trim()` |
| `goal/review/architect.rs:232` | `line.get(3..)` |

**Рефакторинг:** Объединить в `git::parse_porcelain_line`.

---

### P1.4 `DoneContract` save-паттерн — 4 копии

`ralph/engine.rs` (строки 168, 322, 363) и `autopilot/engine/mod.rs` (строка 238) повторяют:
```rust
let mut contract = DoneContract::new(&name, "ralph|autopilot", timestamp);
contract.gates = ...;
contract.passed = ...;
contract.changed_files = detect_changed_files(dir).await;
contract.save(&state_dir.join("done-contract.json")).await?;
```

**Рефакторинг:**
```rust
impl DoneContract {
    async fn from_run(name: &str, mode: &str, gates: &[GateResult], dir: &Path) -> Result<Self> { ... }
    async fn save_to(&self, state_dir: &Path) -> Result<()> { ... }
}
```

---

### P1.5 `which("kimi")` — 5 копий

| Файл | Строка |
|---|---|
| `wire_worker/loop_impl.rs` | 39 |
| `goal/dispatch/runtime.rs` | 11 |
| `cli/team/run.rs` | 51 |
| `cli/goal/commands/run.rs` | 128 |
| `kimi_native/diagnostics/cli.rs` | 5 |

**Рефакторинг:** `runtime::shell::kimi_bin() -> Result<PathBuf>`.

---

### P1.6 `shell_escape` — тонкая обёртка

**Файл:** `src/runtime/autopilot/helpers.rs:63`
```rust
pub(crate) fn shell_escape(s: &str) -> anyhow::Result<String> {
    crate::runtime::shell::shell_escape(s)
}
```

**Рефакторинг:** Удалить, использовать `runtime::shell::shell_escape` напрямую.

---

## P1 — Dead code (удаление = мгновенный win)

### P1.7 `tempfile` в `[dependencies]` вместо `[dev-dependencies]`

Используется **исключительно** в `#[cfg(test)]`. Перенос экономит compile-time для release builds.

---

### P1.8 `#[allow(dead_code)]` с нулевыми внешними вызовами

| Файл | Символ | Примечание |
|---|---|---|
| `git/parse.rs` | `parse_log`, `parse_remotes`, `parse_has_diff` | Нет вызовов |
| `runtime/shell.rs` | `run_command_with_retry` | Только собственный тест |
| `kimi_native/installer.rs` | `install_user_assets` | Нет вызовов |
| `kimi_native/manifest/ops.rs` | `schema_version`, `verify_checksum` | Нет вызовов |
| `runtime/proof/generator/core.rs` | `from_gate_results` | Нет вызовов |
| `runtime/gates/types.rs` | `DoneContract::load` | Нет вызовов |
| `cli/marketplace.rs` | `MarketSkill` | Не инстанциируется |
| `vis/hud_tui/mod.rs` | `worker_task_map` | Нет вызовов |
| `runtime/goal/types.rs` | `GoalControllerStep` | Нет вызовов |
| `runtime/goal/review/architect.rs` | `ArchitectReviewPass` | Review subsystem dead |
| `runtime/goal/review/performance.rs` | `PerformanceReviewPass` | — |
| `runtime/goal/review/slice.rs` | `SliceReviewContext` | — |
| `runtime/goal/review/pass.rs` | `ReviewPass`, `ReviewPassRegistry` | — |
| `runtime/goal/state/db_store.rs` | `DbGoalStateStore` + 6 методов | Весь модуль zombie |

---

### P1.9 `test_helpers` и `MockLlmClient` в production path

**Файлы:**
- `src/lib.rs:22` — `pub mod test_helpers;` (должно быть `#[cfg(test)]`)
- `src/llm/client/mod.rs:88` — `pub struct MockLlmClient` (должно быть `#[cfg(test)]`)

---

### P1.10 Orphaned модули

| Модуль | LOC | Потребители | Рекомендация |
|---|---|---|---|
| `src/skills/` | 258 | 0 | `pub(crate)` или удалить |
| `src/agents/` | 190 | 0 | `pub(crate)` или удалить |
| `src/notifications/` | 617 | Только `runtime/session.rs` | Перенести в `cli/` вместе с `session.rs` |
| `src/analysis/` | ~200 | `goal/planner/discover.rs` (1 файл) | `pub(crate)` |

---

## P1 — Stringly-typed APIs (type safety)

### P1.11 `TaskId = String`, `TicketId = String`

**Файлы:**
- `src/runtime/scheduler/task.rs:26`
- `src/runtime/escalation/preflight.rs:7`

В отличие от `GoalId` (newtype), `RunId`, `GateId`, `WorkerId` (все newtype в `events/id.rs`).

**Рефакторинг:**
```rust
pub struct TaskId(String);
pub struct TicketId(String);
```

---

### P1.12 Domain concepts как `String`

| Файл | Поле | Есть enum? |
|---|---|---|
| `wire/protocol/event/types.rs:10` | `event_type: String` | Да, `runtime::events::kind::EventKind` |
| `runtime/db/types.rs` | `status: String`, `phase: String`, `kind: String` | Да, `GoalStatus`, `GoalPhase` |
| `runtime/autopilot/types.rs:26` | `phase: String` | Да, `AutopilotPhase` |
| `runtime/events/kind.rs:212` | `status: String` | Комментарий: `// "ready", "not_ready", "failed"` |
| `runtime/goal/decision.rs:15` | `kind: String` | — |
| `runtime/goal/task_graph/model.rs:30` | `kind: String` | — |
| `vis/hud/types.rs:20` | `status: String` | Да, `ProofStatus` |

**Рефакторинг:** DB + Wire слои на enum + `serde(rename)` + `From<String>`/`Display`.

---

## P1 — Когнитивная сложность (большие функции)

### P1.13 `vis/engine/model.rs` — `apply()` (~250 LOC, 22 match arms)

Два `match` подряд:
1. `match &ev { ... 7 arms для escalations ... }`
2. `match ev { ... 15+ arms для state updates ... }`

**Рефакторинг:** Каждый arm → метод `apply_*`:
```rust
match ev {
    EngineEvent::ClassifierDecided { .. } => self.apply_classifier_decided(...),
    EngineEvent::WorkerStarted { .. } => self.apply_worker_started(...),
    ...
}
```

---

### P1.14 `proof/generator/events.rs` — `from_event_list()` (~280 LOC)

```rust
match &event.kind {
    EventKind::RunStarted => { ... }
    EventKind::RunCompleted => { ... }
    EventKind::RunFailed => { ... }
    // ... 9+ arms, каждый с serde_json::from_value
}
```

**Рефакторинг:** Extract `impl ProofBuilder { fn apply_run_started(&mut self, event), ... }`.

---

### P1.15 `ralph/engine.rs` — `run_ralph` (~280 LOC)

Concerns: state init/resume → PRD gen → cost estimate → iteration loop → story selection → gate run → budget check.

**Рефакторинг:** `run_iteration()`, `select_story()`, `evaluate_gates()`.

---

### P1.16 `goal/lifecycle/start.rs` — `execute_goal` / `execute_goal_with_dispatcher` (~350 LOC каждая)

Concerns: validation → worktree setup → worker spawn → watchdog → event wait → cleanup → proof gen.

**Рефакторинг:** `setup_worktree()`, `spawn_workers()`, `run_watchdog()`, `generate_proof()`.

---

## P2 — Быстрые win (15–30 минут)

| # | Действие | Файл | Эффект |
|---|---|---|---|
| 1 | Generic `parse_enum<T: FromStr>` для `parse_goal_status`, `parse_goal_phase`, `parse_delivery_policy`, `parse_merge_policy` | `goal/state/db_store.rs` | −120 LOC |
| 2 | `if self.matches_goal(&goal_id)` вместо 15× `if self.goal_id.as_ref() == Some(&goal_id)` | `vis/engine/model.rs` | −30 LOC |
| 3 | `build_prompt(task)` вместо inline mutable `String` push (строки 66–85) | `wire_worker/task/process.rs` | −20 LOC, +читаемость |
| 4 | `const KIMI_PROMPT_TIMEOUT: Duration = Duration::from_secs(120);` в `runtime::shell` | 6 файлов | −6 магических чисел |
| 5 | `pub(crate)` для `skills`, `agents`, `analysis`, `marketplace`, `notifications` | `lib.rs` | −5 публичных модулей без потребителей |
| 6 | `#[cfg(test)]` на `test_helpers` и `MockLlmClient` | `lib.rs`, `llm/client/mod.rs` | −2 production символа |
| 7 | Удалить `shell_escape` обёртку | `autopilot/helpers.rs` | −3 LOC, −1 indirection |

---

## Проверки, которые ПРОЙДЕНЫ ✅

| Правило | Статус |
|---|---|
| Тонкий `main.rs` | ✅ Только `omk::cli::run().await` |
| `unwrap()`/`expect()` в production | ✅ <5, все с fallback |
| Newtype ID-шники (RunId, GateId, WorkerId) | ✅ В `events/id.rs` |
| Нет `runtime/` → `vis/` импортов | ✅ |

---

## Приоритизированный план рефакторинга

### Фаза 1 — Архитектурный фундамент (1–2 дня)
1. **Разорвать цикл `cli ↔ vis`** — trait-контракты в `vis::shell::types`
2. **Переместить `session.rs` в `cli/`** — удалить `runtime/mod.rs:19`
3. **Вынести cost-логику из `runtime/goal/budget.rs`** — DI через `CostSink`
4. **Сузить public API в `lib.rs`** — `pub` → `pub(crate)` для orphaned модулей

### Фаза 2 — Дедупликация (1 день)
5. Унифицировать `run_kimi` → `runtime::shell::run_kimi_prompt`
6. Удалить дубль `detect_changed_files` в `architect.rs`
7. Унифицировать `which("kimi")` → `runtime::shell::kimi_bin`
8. Extract `DoneContract::from_run` + `save_to`

### Фаза 3 — Когнитивная чистка (2–3 дня)
9. **P0.5** — `wire_worker/task/process.rs`: `handle_event`, `handle_request`, `handle_hook`, `handle_tool_call`
10. **P0.6** — `goal/lifecycle/cleanup.rs`: `handle_delivery_*`, `spawn_post_delivery_tasks`
11. **P0.7** — `goal/state/db_store.rs`: intermediate struct + derive для 26 полей
12. **P1.13** — `vis/engine/model.rs`: `apply_*` методы
13. **P1.14** — `proof/generator/events.rs`: `ProofBuilder` handlers
14. **P1.15** — `ralph/engine.rs`: `run_iteration`, `select_story`, `evaluate_gates`

### Фаза 4 — Dead code (0.5 дня)
15. `tempfile` → `[dev-dependencies]`
16. `#[cfg(test)]` на `test_helpers` и `MockLlmClient`
17. Удалить или скрыть `#[allow(dead_code)]` символы без вызовов
18. Удалить `shell_escape` обёртку

### Фаза 5 — Type safety (1 день)
19. Newtype для `TaskId`, `TicketId`
20. String → enum в DB types и Wire event types
