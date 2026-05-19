# Детальный аудит документации проекта oh-my-kimi

**Дата аудита:** 2026-05-19  
**Ветка:** master (post-merge 432a6e1)  
**Версия проекта:** 0.4.0  

---

## 1. Соответствие требованиям AGENTS.md (модульная архитектура)

Корневой `AGENTS.md` требует в каждом `src/X/`:
- `README.md` — назначение, публичный API, статус, зависимости
- `TODO.md` — текущие задачи, известные пробелы
- `AGENTS.md` — правила редактирования, инварианты

| Модуль | README | TODO | AGENTS | Статус |
|--------|--------|------|--------|--------|
| `src/agents/` | ✅ | ✅ | ❌ | **Нет AGENTS.md** |
| `src/analysis/` | ✅ | ✅ | ❌ | **Нет AGENTS.md** |
| `src/bin/` | ❌ | ❌ | ❌ | **Нет ни одного файла** |
| `src/cli/` | ✅ | ✅ | ✅ | Соответствует |
| `src/cost/` | ✅ | ✅ | ✅ | Соответствует |
| `src/kimi_native/` | ✅ | ✅ | ✅ | Соответствует |
| `src/marketplace/` | ✅ | ✅ | ✅ | Соответствует |
| `src/mcp/` | ✅ | ✅ | ✅ | Соответствует |
| `src/notifications/` | ✅ | ✅ | ✅ | Соответствует |
| `src/runtime/` | ⚠️ | ✅ | ✅ | README неполный (см. ниже) |
| `src/skills/` | ✅ | ✅ | ✅ | Соответствует |
| `src/vis/` | ✅ | ✅ | ✅ | Соответствует |
| `src/wire/` | ✅ | ✅ | ✅ | Соответствует |

**Итог:** 10 из 13 модулей полностью соответствуют. 3 модуля имеют пробелы.

---

## 2. Аудит корневой документации

### 2.1 README.md
- **Статус:** ✅ Актуален, хорошо структурирован.
- **Проблемы:** Незначительные. Feature-status table честно отмечает MVP vs Ready.

### 2.2 ARCHITECTURE.md
- **Статус:** ✅ В целом актуален.
- **Проблемы:** Вступление называет goal controller "early scaffold", но описанные data flow и модули изображают почти зрелую систему (slice execution, integrator PRs, review loops, budget enforcement, deterministic replay). Документация недооценивает текущую зрелость.

### 2.3 CHANGELOG.md
- **Статус:** ✅ Актуален, хорошо организован.
- **[Unreleased]** содержит wire-protocol изменения, соответствующие недавним коммитам.

### 2.4 CONTRIBUTING.md
- **Статус:** ⚠️ Частично устарел.
- **Проблема:** ASCII-дерево структуры проекта **пропускает `src/analysis/` и `src/bin/`**. В prose-списке модулей также отсутствует `src/wire/`.

### 2.5 SPEC.md
- **Статус:** ✅ Актуален как продуктовый контракт.
- **Проблема:** В списке команд присутствует `omk goal merge`, но `TODO.md` не содержит этого пункта вообще — команда либо ещё не реализована, либо не отслеживается.

### 2.6 ROADMAP.md
- **Статус:** ⚠️ Содержит устаревшую ссылку.
- **Проблема:** Stage 5 упоминает Beads/PR как coordination layer, но `CHANGELOG.md` 0.4.0 прямо заявляет: **"Beads is no longer required for multi-agent development or omk goal delivery"**. ROADMAP не отражает переход на worktree/PR-first workflow.

### 2.7 TODO.md
- **Статус:** ⚠️ Фактически исторический документ.
- **Проблема:** Фазы 1–11 и 13 полностью выполнены (все галочки). Активных задач осталось ~5 в "Phase 12 Leftovers". Документ нуждается в реструктуризации: архивировать выполненное, оставить только активный бэклог.

### 2.8 docs/API.md
- **Статус:** ❌ Существенный пробел.
- **Проблема:** Не содержит **ни одного** goal-related MCP tool или REST endpoint, несмотря на то что `omk goal` — north-star feature. Документ застыл на pre-goal MVP стадии (только `omk_team_run`, `omk_team_status`, `omk_team_shutdown`, `omk_doctor`).

### 2.9 docs/TROUBLESHOOTING.md
- **Статус:** ⚠️ Частично устарел.
- **Проблема:** В некоторых разделах смешаны legacy-команды (`omk run show latest`, `omk proof show latest`) с goal-командами. Для пользователей, работающих преимущественно с `omk goal`, это может сбивать с толку.

### 2.10 docs/KIMI_UPSTREAM.md
- **Статус:** ⚠️ Устарел.
- **Проблемы:**
  - "Last checked: 2026-05-09" — 10 дней назад.
  - Все 4 пункта release checklist не отмечены (unchecked).

### 2.11 docs/REGISTRY.md
- **Статус:** ✅ Актуален, краткий и чёткий.

### 2.12 docs/GOAL_NOTIFICATIONS.md
- **Статус:** ✅ Актуален, хорошо определён контракт.

### 2.13 docs/PROJECT_MAP.md
- **Статус:** ✅ Актуален, честные метки зрелости.

---

## 3. Аудит модульной документации

### 3.1 `src/runtime/README.md` — ❌ Наиболее проблемный
- **Нет перечисления публичного API.** Только file map.
- **Нет секции dependencies.**
- **Нет явного status:** поля.
- **File map устарел:** упоминает корневые файлы (`autopilot.rs`, `events.rs`, `gates.rs`, `proof.rs`, `ralph.rs`, `wire_worker.rs`), но **не упоминает одноимённые поддиректории** (`src/runtime/autopilot/`, `src/runtime/events/`, `src/runtime/gates/`, `src/runtime/proof/`, `src/runtime/ralph/`, `src/runtime/scheduler/`, `src/runtime/ask/`, `src/runtime/wire_worker/`, `src/runtime/goal/`), созданные в результате рефакторинга #93.

### 3.2 `src/wire/README.md` — ⚠️ Неполный
- **Пустые списки зависимостей** (`internal: []`, `external: []`). Для протокольного/клиентского модуля это явно некорректно — wire зависит от `serde`, `tokio`, `anyhow` и др.

### 3.3 `src/mcp/README.md` — ⚠️ Неточная ссылка
- Указывает `src/wire/client.rs` как consumer, использующий `mcp::server`. На самом деле `src/wire/client.rs` содержит только doc-comment упоминание — нет реального импорта или кода-зависимости.

### 3.4 `src/analysis/README.md` — ⚠️ Неполный
- **Нет секции dependencies.**
- **Public API** перечисляет 3 функции, но опускает тип `SyntaxTree` и трейты.

### 3.5 `src/bin/` — ❌ Полностью отсутствует документация
- Нет `README.md`, `TODO.md`, `AGENTS.md`.
- Единственный файл — `validate-contracts.rs` (352 строки, >400-line limit, но это binary crate, не library module).
- Согласно корневому `AGENTS.md`, `src/main.rs` должен оставаться thin dispatch only. Для `src/bin/` стоит добавить минимальный `README.md` с пояснением конвенции.

### 3.6 `src/agents/` и `src/analysis/` — ⚠️ Нет AGENTS.md
- Остальные 11 модулей имеют `AGENTS.md`. Эти два — нет.

### 3.7 Остальные модули
- `src/cli/`, `src/cost/`, `src/kimi_native/`, `src/marketplace/`, `src/notifications/`, `src/skills/`, `src/vis/` — ✅ Хорошие README, TODO, AGENTS.

---

## 4. Аудит `docs/superpowers/`

### 4.1 `specs/2026-05-11-omk-goal-design.md`
- **Статус:** В основном реализовано; дизайн валиден.
- **Проблема:** Секция "Later commands" (`plan`, `approve-plan`, `open-pr`) устарела — `plan` и `open-pr` уже реализованы.

### 4.2 `specs/2026-05-14-omk-goal-end-to-end-delivery.md`
- **Статус:** ✅ Активен и релевантен. Корректно описывает незакрытый gap (Phase 12 Leftovers в TODO.md).

### 4.3 `specs/2026-05-15-code-quality-cleanup-design.md`
- **Статус:** ⚠️ Устарел в спецификах.
- **Проблема:** Таблица "Files >400 Lines" перечисляет 16 файлов, но **13 из них уже разбиты на директории** коммитом `07c47c6` и ранее. Осталось только 4 файла >400 строк.

### 4.4 `plans/2026-05-12-*` (12 файлов)
- **Статус:** ⚠️ Исторические записи.
- **Проблема:** Все 12 планов имеют 100% отмеченных чекбоксов. Они являются точными историческими записями, но **не должны восприниматься как активный бэклог**. Стоит явно пометить их как COMPLETED.

### 4.5 `plans/2026-05-15-code-quality-cleanup.md`
- **Статус:** ❌ Опасен для использования.
- **Проблема:** Ни один чекбокс не отмечен. Инструкции ссылаются на файлы, которые **больше не существуют** (`src/runtime/goal/dispatch.rs`, `src/runtime/autopilot/engine.rs`, `src/cli/app.rs`, `src/kimi_native/manifest.rs`, `src/wire/protocol/event.rs` и др. — все уже разбиты на подмодули). Агент, следуя этому плану дословно, потерпит неудачу.

---

## 5. Консистентность документации ↔ код

### 5.1 Жёсткие правила AGENTS.md vs реальность

| Правило | Декларация | Реальность в `src/` | Статус |
|---------|-----------|---------------------|--------|
| `unwrap()` banned | "Use `?`, `if let`, `match`..." | **643** вызова | ❌ Массовые нарушения |
| `expect()` banned | "No 'this should never happen'" | **90** вызовов | ❌ Массовые нарушения |
| `panic!()` banned | "Graceful degradation only" | **17** вызовов | ❌ Нарушения |
| `TODO/FIXME/HACK` в production | "Zero... in production code" | **0** (только в `slop.rs` как паттерн для сканирования) | ✅ Соблюдается |

> **Примечание:** Согласно `AGENTS.md`, правила 1–3 применяются к **new or modified production code**, а не требуют полного ретро-рефакторинга legacy-кода. Тем не менее, 643 `unwrap()` — это существенный техдолг, который не отражён в документации.

### 5.2 Версии

| Источник | Значение | Статус |
|----------|----------|--------|
| `Cargo.toml` | `0.4.0` | ✅ Source of truth |
| `VERSION` | `0.4.0` | ✅ Соответствует |
| `homebrew/omk.rb` | `0.4.0` | ✅ Соответствует |
| `aur/PKGBUILD` | `0.4.0` | ✅ Соответствует |
| `flake.nix` | `0.4.0` | ✅ Соответствует |
| `install.sh` (комментарий) | `0.3.30` | 🟡 Косметически устарел |
| `scripts/sync-packaging-versions.sh` (пример) | `0.3.30` | 🟡 Косметически устарел |

### 5.3 `.omk/AGENTS.md` — Wire Protocol
- **Протокол:** Указывает версию 1.9, `kimi` 1.41.0. Соответствует `src/wire/protocol.rs`.
- **Проблема:** Roadmap внутри `.omk/AGENTS.md` показывает два неотмеченных пункта:
  - `Approval proxy (OMK approves/rejects on behalf of user)` — unchecked
  - `Hook integration (OMK hooks via wire HookRequest)` — unchecked
  Это корректно отражает текущее состояние.

---

## 6. Приоритетные рекомендации

### 🔴 Критический (блокирует корректность)

1. **Обновить `plans/2026-05-15-code-quality-cleanup.md`** — содержит инструкции для несуществующих файлов. Либо удалить, либо полностью переписать с актуальными путями и метриками.
2. **Добавить `src/bin/README.md`** — пояснить, что `src/bin/` содержит вспомогательные бинарники, а `src/main.rs` — thin dispatch.
3. **Переписать `src/runtime/README.md`** — добавить public API surface, dependencies, explicit status, обновить file map с учётом поддиректорий.

### 🟡 Высокий (улучшает точность)

4. **Дополнить `docs/API.md`** goal-related MCP tools и REST endpoints (`omk goal run`, `omk goal status`, `omk goal proof` и т.д.).
5. **Обновить `ROADMAP.md`** Stage 5 — убрать/уточнить упоминания Beads, отразить worktree/PR-first workflow.
6. **Реструктуризировать `TODO.md`** — архивировать выполненные фазы 1–11 и 13, оставить только Phase 12 Leftovers как активный бэклог.
7. **Добавить `AGENTS.md` в `src/agents/` и `src/analysis/`**.

### 🟢 Средний (поддержание свежести)

8. **Обновить `docs/KIMI_UPSTREAM.md`** — обновить дату проверки, пройти release checklist.
9. **Исправить `CONTRIBUTING.md`** — добавить `src/analysis/` и `src/bin/` в дерево проекта.
10. **Обновить `SPEC.md`** — либо реализовать `omk goal merge`, либо убрать из списка команд.
11. **Пометить `plans/2026-05-12-*` как COMPLETED**.
12. **Обновить `specs/2026-05-15-code-quality-cleanup-design.md`** — пересчитать файлы >400 строк и `unwrap/expect/panic` counts.
13. **Исправить `src/wire/README.md`** — заполнить списки зависимостей.
14. **Исправить `src/mcp/README.md`** — убрать или скорректировать ссылку на `src/wire/client.rs` как consumer.
15. **Привести `docs/TROUBLESHOOTING.md` к goal-first стилю** — минимизировать смешение legacy-команд.

---

## 7. Метрики аудита

| Категория | Количество | Примечание |
|-----------|-----------|------------|
| Всего .md файлов в проекте | 94+ | Включая корень, docs/, src/, .omk/ |
| Модулей src/X/ | 13 | |
| Модулей с полным комплектом (README+TODO+AGENTS) | 10/13 | 77% |
| Корневых документов с проблемами | 6/13 | ~46% |
| Реализованных, но не архивированных планов | 12 | В docs/superpowers/plans/ |
| Устаревших спеков с неверными путями | 2 | code-quality-cleanup design + plan |
| unwrap() в production | 643 | Техдолг не отражён в документации |
| expect() в production | 90 | Техдолг не отражён в документации |
| panic! в production | 17 | Техдолг не отражён в документации |
