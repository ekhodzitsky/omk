# Детальный аудит кодовой базы oh-my-kimi

**Дата:** 2026-05-27  
**Версия:** omk 0.5.0  
**Scope:** `src/**/*.rs`, `tests/**/*.rs` против `AGENTS.md`  
**Компиляция:** `cargo check --all-features` → 0 ошибок, 0 предупреждений  
**Тесты:** `cargo test --lib --all-features` → 757 passed, 0 failed

---

## Исправления, применённые в этой сессии

| # | Проблема | Файлы | Статус |
|---|---|---|---|
| P0.1 | `MutexGuard` удерживался через `await` в цикле Wire-сообщений | `src/llm/client/mod.rs` | ✅ Исправлено — guard теперь берётся только на одну async операцию |
| P0.2 | `Command::spawn/output` без `kill_on_drop`/`process_group` | `src/runtime/ultrawork.rs`, `ralph/runner.rs`, `autopilot/helpers.rs`, `autopilot/engine/phases.rs`, `gates/detect.rs`, `ask/provider.rs`, `goal/oracle/rewrite.rs` | ✅ Исправлено — все через `configure_command` |
| P0.4 | Flaky test `manifest_init_and_load` — фиксированный `run_id` в глобальном state_dir | `src/runtime/scheduler/manifest.rs` | ✅ Исправлено — UUID + cleanup |
| P0.5 | Prometheus экспортировал только 5 из 11 метрик | `src/vis/server/handlers.rs` | ✅ Исправлено — добавлены `tasks_completed`, `tasks_failed`, `ask_errors`, `autopilot_runs`, `ralph_runs` |

**Остался открытым:** P0.3 — Dual state persistence (JSON ↔ SQLite). Требует архитектурного решения (выбор единственного источника истины), не может быть исправлен одним коммитом.

---

---

## Методология

Аудит проведён по 5 направлениям:
1. **Async Safety** — mutex через await, blocking ops, std::fs в async, timeouts
2. **Error Handling** — unwrap/expect/panic, anyhow vs thiserror, silent errors
3. **Security** — path traversal, process_group(0), kill_on_drop, secrets
4. **Observability** — tracing::instrument, structured fields, metrics
5. **Architecture & Tests** — module boundaries, flaky tests, coverage gaps

---

## P0 — Критические нарушения (безопасность / data race / deadlock)

### P0.1 `llm/client/mod.rs` — MutexGuard удерживается через цикл с await
**Файл:** `src/llm/client/mod.rs:198–259`  
**Описание:** `WireLlmClient::complete_inner` захватывает `let mut wire = self.wire.lock().await;` и держит guard на протяжении всего цикла чтения Wire-сообщений, включая `wire.start_prompt().await` и `wire.read_message().await` внутри `tokio::time::timeout`. Это блокирует mutex на всё время LLM-вызова (секунды–минуты). Любой другой task, пытающийся использовать тот же `WireLlmClient`, будет бесконечно ждать.
```rust
let (content, status_tokens) = {
    let mut wire = self.wire.lock().await;  // ← guard taken
    let id = wire.start_prompt(prompt).await?;  // ← held across await
    loop {
        let msg = tokio::time::timeout(..., wire.read_message()).await?;  // ← held across await
        ...
    }
};
```
**Исправление:** Вынести `start_prompt` и `read_message` за пределы scoped блока, либо перепроектировать `WireLlmClient` так, чтобы `Arc<Mutex<W>>` не нужен был для последовательных операций (если `W: Send`, передавать `&mut W` напрямую).

---

### P0.2 Multiple `Command::spawn` без `kill_on_drop(true)` и `process_group(0)`
**Файлы:**
| Файл | Строка | Команда |
|---|---|---|
| `src/runtime/ultrawork.rs` | 160 | `kimi -p` |
| `src/runtime/ralph/runner.rs` | 13, 34 | `kimi -p`, `cargo test` |
| `src/runtime/autopilot/helpers.rs` | 34, 53 | `kimi --print`, произвольная команда |
| `src/runtime/autopilot/engine/phases.rs` | 124 | `omk team run` |
| `src/runtime/gates/detect.rs` | 112 | `git status` |
| `src/runtime/ask/provider.rs` | 21 | `which` |
| `src/runtime/goal/oracle/rewrite.rs` | 52 | произвольная команда (kill_on_drop есть, process_group нет) |

**Описание:** Все эти вызовы используют `.output()` без `.kill_on_drop(true)` и без `process_group(0)`. При `Ctrl+C` или drop future дочерний процесс и его внуки останутся зомби. `oracle/rewrite.rs` имеет `kill_on_drop(true)`, но не `process_group(0)`.
**Исправление:** Применить `crate::runtime::shell::configure_command(&mut cmd)` перед `.output()`/`.spawn()` во всех файлах.

---

### P0.3 Dual-write goal state — JSON vs SQLite race
**Файл:** `src/runtime/goal/state/store.rs`  
**Описание:** `FileSystemGoalStateStore::save()` пишет **и** в SQLite, **и** в JSON. `load()` читает **JSON first**. `list()` читает **SQLite first**. Это означает:
- `save()` → DB + JSON
- `load()` → JSON (может проигнорировать более свежие данные в DB)
- `list()` → DB (может показывать цели, чей JSON был удалён)

**Исправление:** Выбрать единственный источник истины. Если SQLite — удалить JSON fallback в `load()`. Если JSON — удалить DB path в `list()`.

---

### P0.4 Flaky test `manifest_init_and_load` — гонка за глобальный state_dir
**Файл:** `src/runtime/scheduler/manifest.rs:244–255`  
**Описание:** Тест использует `run_id = "run-test-123"` и пишет/читает из глобального `state_dir()`. При параллельном выполнении (cargo nextest) другой инстанс теста или другой тест может перезаписать/удалить те же файлы.
**Исправление:** Принимать `state_dir: &Path` как параметр в `RunManifest::load` и использовать `TempDir` в тесте.

---

### P0.5 Prometheus handler экспортирует только 5 из 11 метрик
**Файл:** `src/vis/server/handlers.rs`  
**Описание:** `Metrics` имеет 11 полей, но handler выводит только 5. Отсутствуют:
- `total_tasks_completed`
- `total_tasks_failed`
- `total_ask_errors`
- `total_autopilot_runs`
- `total_ralph_runs`

**Исправление:** Добавить оставшиеся 6 метрик (с учётом legacy alias `total_spawns` → `total_team_runs`) в строку ответа Prometheus.

---

## P1 — Политические нарушения AGENTS.md

### P1.1 Zero `#[tracing::instrument]` на ~286 `pub async fn`
**Scope:** Весь `src/`  
**Находка:** `grep -rn '#\[instrument' src/ --include='*.rs'` → 0 совпадений.  
**Влияние:** В production невозможно отследить `run_id`, `goal_id`, `worker_id` через распределённые трейсы. Все spawned tasks — orphaned traces.

### P1.2 Format-string tracing вместо structured fields (~22 call sites)
**Файлы:** `src/runtime/ralph/engine.rs`, `src/runtime/autopilot/`, `src/runtime/ultrawork.rs`, `src/vis/server/bootstrap.rs`, `src/wire/client/spawn.rs`
**Пример:** `info!("Estimated cost: {}", rough_estimate.formatted())` → должно быть `info!(cost = %rough_estimate.formatted(), "estimated cost")`.

### P1.3 `println!` / `eprintln!` в library code
**Файл:** `src/skills/parser.rs:81–82`  
```rust
eprintln!("FM: {:?}", fm);
eprintln!("BODY: {:?}", body);
```
Должно быть `tracing::debug!`.

### P1.4 Bare `tokio::spawn` без stored `JoinHandle` (~16 мест)
**Файлы:** `src/wire/client/spawn.rs`, `src/runtime/wire_worker.rs`, `src/runtime/events/writer.rs`, `src/runtime/goal/supervisor.rs`, `src/runtime/goal/chat_api/`, `src/mcp/client/transport.rs`, `src/mcp/client/http_transport.rs`
**Правило:** AGENTS.md §1 требует stored handles или `JoinSet`.

### P1.5 `let _ = ...` on Result без обоснования
**Файлы:** `src/runtime/session.rs`, `src/runtime/ask/execution.rs`, `src/runtime/classifier/mod.rs`, `src/runtime/db/transaction.rs`, `src/runtime/goal/git_ops/auto_rebase.rs`, `src/runtime/goal/dispatch/`, `src/runtime/goal/open_pr.rs`
**Пример:** `let _ = tokio::fs::remove_dir_all(&tmp_dir).await;` — ошибка удаления temp dir игнорируется.

### P1.6 `anyhow::Result` в public library API
**Файлы:** `src/wire/client.rs`, `src/cost/sink.rs`, `src/cost/tracker.rs`, `src/runtime/gates/types.rs`, `src/runtime/proof/types.rs`, `src/runtime/scheduler/*.rs`, `src/runtime/ralph/*.rs`, `src/runtime/classifier/telemetry.rs`
**Правило:** Library code (вне `src/cli/`) должен использовать `thiserror` typed enums.

### P1.7 Архитектурные нарушения границ модулей
- `src/runtime/goal/budget.rs` → импортирует `crate::cost::*` (runtime → cost)
- `src/runtime/session.rs` → импортирует `crate::cost::*`, `crate::notifications::*` (runtime → cost/notifications)
- `src/vis/shell/` → импортирует `crate::cli::chat::*` (vis → cli)

### P1.8 `unimplemented!()` в production
**Файл:** `src/runtime/goal/merge.rs:99,106`
```rust
_ => unimplemented!("merge policy {policy:?}"),
```
Должно быть typed error.

### P1.9 `Proof::write_json` — sync I/O рядом с async `save`
**Файл:** `src/runtime/proof/types.rs:117`  
`pub fn write_json(&self, path: &Path)` использует `std::fs::write`. Рядом `pub async fn save`. Смешение sync/async I/O для одного типа.

### P1.10 `tokio::spawn` без `.instrument()` — потеря span context
Все ~16 spawned tasks (см. P1.4) не используют `.in_current_span()` или `.instrument(info_span!(...))`. В distributed tracing они появляются как orphaned root spans.

---

## P2 — Гигиена

### P2.1 Orphaned module `src/skills/`
4 файла, минимальное потребление (только `cli/kimi_native_cmd/` и `kimi_native/diagnostics/`). Нет `README.md` с описанием границ.

### P2.2 `DbGoalStateStore` — mostly `#[allow(dead_code)]`
**Файл:** `src/runtime/goal/state/db_store.rs`  
Весь SQLite path отмечен `allow(dead_code)`, но компилируется и тянет `tokio-rusqlite`.

### P2.3 `tempfile::tempdir()` в ~80+ unit tests
AGENTS.md Tier 1: "No temp files in unit tests". Многие `#[cfg(test)]` в `src/` используют `tempfile` вместо in-memory mocks.

### P2.4 Отсутствие property / snapshot / DST tests
- **Tier 2 (proptest):** 0 тестов
- **Tier 3 (insta):** 0 тестов
- **Tier 5 (shuttle/turmoil):** 0 тестов

### P2.5 Nested modules без README.md / TODO.md / AGENTS.md
42 вложенных подмодуля (например, `src/runtime/goal/planner/`, `src/wire/protocol/`) не имеют локальной документации.

---

## Сводная таблица

| Категория | P0 | P1 | P2 |
|---|---|---|---|
| Async Safety | 2 | 1 | 0 |
| Error Handling | 0 | 3 | 0 |
| Security / Process | 1 | 0 | 0 |
| Observability | 1 | 3 | 0 |
| Architecture | 1 | 2 | 2 |
| Testing | 1 | 1 | 3 |
| **Итого** | **6** | **10** | **5** |

---

## Рекомендуемый порядок исправлений

### Спринт 1 (безопасность + стабильность)
1. **P0.1** — Переписать `WireLlmClient::complete_inner` без удержания mutex guard через await.
2. **P0.2** — Добавить `configure_command` ко всем `Command::new(...).output()` в `ultrawork`, `ralph`, `autopilot`, `gates/detect`, `ask/provider`.
3. **P0.4** — Исправить flaky test `manifest_init_and_load`.

### Спринт 2 (архитектура + данные)
4. **P0.3** — Выбрать единственный источник истины для goal state (JSON или SQLite).
5. **P1.7** — Вынести `runtime/session.rs` в `cli/`, ввести `CostSink` trait для `runtime/goal/budget.rs`.

### Спринт 3 (observability)
6. **P0.5** — Добавить недостающие 6 метрик в Prometheus handler.
7. **P1.1** — Добавить `#[tracing::instrument]` на топ-20 `pub async fn` (wire, goal lifecycle, scheduler, gates).
8. **P1.2** — Исправить format-string tracing в `ralph/`, `autopilot/`, `ultrawork/`.
9. **P1.4** / **P1.10** — Добавить `JoinSet`/stored handles и `.instrument()` на spawned tasks.

### Спринт 4 (гигиена)
10. **P1.5** — Заменить `let _ =` на explicit error handling.
11. **P1.8** — Заменить `unimplemented!` на typed error.
12. **P1.6** — Постепенно заменять `anyhow::Result` на `thiserror` в public library API.
13. **P2.3–P2.4** — Добавить baseline property/snapshot tests для Wire protocol types.
