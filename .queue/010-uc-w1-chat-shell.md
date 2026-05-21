---
id: 010
title: UNIFIED_CHAT W1 — TUI chat shell foundation
status: todo
branch: feat/chat-shell
worktree: .worktrees/unified-chat-W1-shell
blocked_by: []
merge_after: []
size: large
batch: unified-chat-wave-1
pr: null
notes: Spec §13 W1. Pure TUI, no LLM logic. Stub-echo responses. ratatui-based.
---

# Prompt

Ты работаешь в worktree `.worktrees/unified-chat-W1-shell` на ветке feat/chat-shell (создана от master @ 8425033). Workstream: W1 — Shell foundation.

Часть волны UNIFIED CHAT — нового CLI `omk` с TUI и встроенной оркестрацией. Твой workstream — фундамент: TUI-shell без LLM-логики и оркестрации. Чистый терминал, который принимает текст и отвечает stub-эхом.

==================================================================
АБСОЛЮТНЫЕ ЗАПРЕТЫ
==================================================================

§12 ANTI-GOALS (нарушение = блок PR):
1.  No silent side effects — каждая запись на диск анонсируется.
2.  No vendor lock-in beyond Kimi.
3.  No cloud control plane.
4.  No telemetry transmission.
5.  No web UI — терминал-native.
6.  No copying competitor code.
7.  No stub commands — slash-команды /quit / /theme / /help работают ПОЛНОСТЬЮ. Stub-echo вместо LLM в conversation log — специальная заглушка LLM-ответа, это OK.
8.  No deprecation of `omk goal run` headless.
9.  No magic without proof.
10. No mandatory account.

§14 COORDINATION:
- ТЫ МОЖЕШЬ ИЗМЕНЯТЬ ТОЛЬКО:
    src/cli/chat/      (новый, создаёшь)
    src/vis/shell/     (новый, создаёшь)
    tests/chat_shell_integration_test.rs
- ТЫ НЕ ТРОГАЕШЬ: Cargo.toml, src/lib.rs, src/main.rs, src/cli/mod.rs, README.md, CHANGELOG.md, docs/UNIFIED_CHAT*.md, и любые файлы вне owned-paths.

ВНУТРИ owned-paths ты свободен создавать любые submodules.

==================================================================
ЗАДАЧА
==================================================================

1. СПЛИТ-РАЗМЕТКА: левая часть — conversation log. Правая часть — engine pane (заглушка для W4).

2. ENGINE PANE СОСТОЯНИЯ:
   - collapsed: одна строка `[engine] session: <id> · idle · cost: $0.00 · Tab to expand`
   - compact: 8-12 строк (заглушка "no events yet")
   - expanded: полная высота, тоже заглушка
   - Tab toggles compact <-> expanded
   - Shift-Tab → collapsed
   - Auto-collapse через 5 минут idle (для W1: idle = нет нажатий)

3. МОДАЛЬНЫЙ ВВОД (vi-стиль без normal mode):
   - Text mode (default): печатаем в input box. Enter — отсылает. Shift-Enter — newline.
   - Command mode: триггер = первый '/'. Tab-completion для имён. Esc → text mode.

4. HOTKEYS: Tab, Shift-Tab, Ctrl-L (clear view), Ctrl-D/Ctrl-C (quit с confirm), Up/Down (history), PgUp/PgDn (scroll), Esc.

5. ТЕМЫ: Dark (default), Light. /theme dark|light. Используй ratatui::style — не хардкодь ANSI.

6. ПЕРСИСТЕНТНОСТЬ:
   Путь: ~/.local/state/omk/sessions/<session_id>/
   session_id формат: `o7k_<8-char-base32>` (алфавит без I, O, 1, 0)
   Файлы:
     conversation.jsonl — append-only, line-atomic. JSON:
       { "ts": "2026-05-21T08:14:32.123Z",
         "role": "user" | "assistant",
         "text": "..." }
     meta.json — { "session_id", "started_at", "project_root", "last_activity",
                   "theme", "schema_version": 1 }
     engine-events.jsonl — append-only, для W1 пустой.

7. ВХОД `omk` без аргументов:
   pub fn run_chat(args: ChatArgs) -> anyhow::Result<()> в src/cli/chat/run.rs.
   main.rs НЕ трогаешь — в PR description: "orchestrator please wire src/main.rs to call cli::chat::run::run_chat() when argv.len() == 1".
   `omk --new` — новая сессия.
   `omk --session <id>` — резюмируешь.
   Без флагов — резюмируешь последнюю в этом project_root (`git rev-parse --show-toplevel`).

8. STUB ECHO: assistant-ответ = `[W1 stub] received "<первые 60 chars>"`. Через conversation.jsonl запись.

9. ОНБОРДИНГ-HINT (§3.1): при первой сессии (по ~/.config/omk/seen.json) показать в правой колонке `[Press Tab to see what's happening under the hood]`. После первого Tab — записать seen.json: {"tab_hint": true}, больше не показывать.

==================================================================
РАЗВЕДКА
==================================================================

1. Cargo.toml — есть ли ratatui, crossterm, tokio, anyhow, serde, serde_json, chrono или time. Если нет — запроси в PR description.
2. src/vis/hud_tui/ — паттерн ratatui-based TUI: layout, focus, event loop.
3. src/runtime/events/writer.rs — line-atomic JSONL pattern.
4. src/cli/goal/ — структура CLI-модуля.

==================================================================
СКЕЛЕТ
==================================================================

src/cli/chat/mod.rs:
    pub mod app;
    pub mod commands;
    pub mod input;
    pub mod persistence;
    pub mod run;
    pub mod session_id;

src/cli/chat/run.rs:
    use anyhow::Result;
    use clap::Args;

    #[derive(Args, Debug, Clone)]
    pub struct ChatArgs {
        #[arg(long)]
        pub session: Option<String>,
        #[arg(long)]
        pub new: bool,
    }

    pub fn run_chat(args: ChatArgs) -> Result<()> {
        // setup terminal (crossterm raw mode)
        // resolve session_id
        // build app state
        // run event loop until exit
        // teardown
    }

src/vis/shell/mod.rs:
    pub mod layout;
    pub mod theme;
    pub mod engine_placeholder;
    pub mod conversation_view;
    pub mod input_view;

==================================================================
ТЕСТЫ (минимум 5)
==================================================================

tests/chat_shell_integration_test.rs:
- test_app_starts_with_collapsed_engine_pane
- test_tab_expands_engine_pane_then_back_to_compact
- test_shift_tab_collapses_engine_pane
- test_text_input_appends_to_conversation
- test_session_resume_loads_persisted_conversation

Тестовый подход: вызывай cli::chat::app::App::handle_event(...) напрямую с синтетическими KeyEvents, проверяй state. Если для test нужен spawn binary `omk` — #[ignore = "blocked on src/main.rs wiring by orchestrator"], опиши в PR body.

==================================================================
SUCCESS CRITERIA
==================================================================

- cargo build --workspace зелено
- cargo test --test chat_shell_integration_test зелено
- cargo test --workspace — ничего смежного не сломано
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status: только src/cli/chat/** и src/vis/shell/** и tests/chat_shell_integration_test.rs
- PR title: "feat(chat): TUI shell foundation (W1)"
- PR body содержит: требуемые deps, запрос на wire в src/main.rs (точная сигнатура), запрос на pub mod chat; в src/cli/mod.rs, ссылку на docs/UNIFIED_CHAT_DECISIONS.md, список тестов

==================================================================
СТОП-ТРИГГЕРЫ
==================================================================

- ratatui отсутствует в Cargo.toml — STOP, опиши.
- src/vis/hud_tui использует другую TUI-библиотеку — STOP, уточни.
- Нужно править src/main.rs — НЕ ПРАВЬ, #[ignore] тест с пояснением.
- src/cli/chat/ или src/vis/shell/ уже существуют — STOP (не должно быть на base 8425033).

НЕ ПЕРЕСМАТРИВАЙ §3 (UX), §7 (engine pane), §8 (control surface), §9 (state model), §13, §14. Ship velocity > polish.

Начинай с разведки.
