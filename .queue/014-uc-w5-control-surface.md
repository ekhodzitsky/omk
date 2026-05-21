---
id: 014
title: UNIFIED_CHAT W5 — slash commands, hotkeys, preflight keys
status: todo
branch: feat/control-surface
worktree: .worktrees/unified-chat-W5-control-surface
blocked_by: []
merge_after: [010, 012, 015]
size: large
batch: unified-chat-wave-1
pr: null
notes: Spec §13 W5 + §8. Parsing complete and tested; backend dispatch via trait with StubBackend until W3/W6 ready. Owns src/vis/shell/keymap.rs as a one-file exception inside W1's tree.
---

# Prompt

Ты работаешь в worktree `.worktrees/unified-chat-W5-control-surface` на ветке feat/control-surface. Workstream: W5 — Control surface.

Input router внутри shell: все slash-команды, все hotkey, modal handling. Парсинг полный и тестируемый; диспатч к backend (W3/W6) — через trait/API.

==================================================================
АБСОЛЮТНЫЕ ЗАПРЕТЫ
==================================================================

§12 ANTI-GOALS — особенно:
7. No stub commands. Каждая команда из §8.2 либо:
   - выполняется через backend trait — production-grade,
   - возвращает структурированное "not yet wired by orchestrator (W3 not merged)" — парсер для неё полный и тестированный.
   Это ОК. НЕ ОК: команда "тихо ничего не делает".

§14 COORDINATION:
- ТЫ МОЖЕШЬ ИЗМЕНЯТЬ ТОЛЬКО:
    src/cli/chat/commands/      (новый)
    src/vis/shell/keymap.rs     (один файл; coordination с W1)
    tests/control_surface_test.rs
- ТЫ НЕ ТРОГАЕШЬ остальное в src/cli/chat/ (W1) или src/vis/shell/ (W1).
- Cargo.toml, src/lib.rs, src/main.rs, src/cli/mod.rs — не трогаешь.

NB: keymap.rs — этот файл живёт ВНУТРИ src/vis/shell/ (W1's tree). Чтобы не конфликтовать с W1: ты создаёшь ТОЛЬКО keymap.rs (один файл), W1 в своём mod.rs добавит pub mod keymap; через orchestrator на coordination day. Если src/vis/shell/ ещё не существует (W1 не смержен) — создай:
    src/vis/shell/keymap.rs
И в PR description проси orchestrator'а интегрировать с W1.

==================================================================
ЗАДАЧА
==================================================================

1. SLASH COMMANDS (§8.2) — полный список:
   /help, /quick <prompt>, /escalate <prompt>, /classify <prompt>, /explain,
   /show plan, /show proof, /show goals, /goal show <id>, /inject <text>,
   /pause, /resume, /cancel, /approve, /reject [reason], /diff, /cost,
   /new, /sessions, /resume <id> (с аргументом — session resume; без — unpause workers),
   /theme dark | /theme light, /quit

2. COMMAND PARSER:
        pub struct Command {
            pub name: String,
            pub args: Vec<String>,
        }
        pub fn parse_command(input: &str) -> Result<Command, ParseError>;

   Правила: input начинается с '/', первое слово = name (без слэша), остальное = args (split by whitespace, quoted strings — одним токеном; используй shlex если в Cargo.toml; иначе минимальный собственный).
   Tab completion: pub fn complete(prefix: &str) -> Vec<&'static str>.

3. COMMAND REGISTRY:
        pub struct CommandSpec {
            pub name: &'static str,
            pub aliases: &'static [&'static str],
            pub help: &'static str,
            pub args_help: &'static str,
            pub min_args: u8,
            pub max_args: Option<u8>,
        }
        pub const COMMAND_REGISTRY: &[CommandSpec] = &[ ... все 20+ ... ];
   /help рендерит таблицу из реестра.

4. COMMAND DISPATCH:
        pub trait CommandBackend: Send + Sync {
            async fn dispatch_quick(&self, prompt: &str) -> CommandResponse;
            async fn dispatch_escalate(&self, prompt: &str) -> CommandResponse;
            async fn dispatch_classify(&self, prompt: &str) -> CommandResponse;
            // ... все 20+ команд
        }
   StubBackend impl возвращает "not yet wired (requires W3 router; not yet merged)". Тесты используют StubBackend.

5. UNKNOWN COMMAND HANDLING (D8):
   '/' но не в реестре:
   - Первый раз за сессию: hint "[that's not a command; sending as text. use /help for available commands.]"
   - Последующие: тихо как text.
   SessionCtx::unknown_command_hinted: bool.

6. HOTKEYS (§8.3):
   Tab → ToggleEnginePane(Compact <-> Expanded)
   Shift+Tab → ToggleEnginePane(Collapsed)
   Ctrl-P → /pause
   Ctrl-R → /resume (без аргументов — unpause)
   Ctrl-K → /cancel
   Ctrl-I → open inline inject input
   Ctrl-A → /approve
   Ctrl-J → prompt for reason then /reject
   Ctrl-L → ClearConversationView
   Ctrl-D, Ctrl-C → quit (confirm if active goal)
   Up/Down → input history navigation
   PgUp/PgDn → scroll conversation
   Esc → exit command mode / close inline dialog

   pub fn map_key_to_action(key: KeyEvent, ctx: &InputContext) -> Option<Action>;

7. PREFLIGHT-DIALOG KEYS (§8.4):
   Когда preflight активен (flag в InputContext):
   Enter → PreflightAccept
   E     → PreflightExplain
   Q     → PreflightDowngrade
   Esc   → PreflightCancel
   Остальные клавиши блокируются до закрытия preflight.

==================================================================
ТЕСТЫ (минимум 15)
==================================================================

tests/control_surface_test.rs:
- test_parse_command_simple
- test_parse_command_with_args
- test_parse_command_with_quoted_arg
- test_parse_command_empty_after_slash_returns_error
- test_tab_completion_for_partial_command
- test_help_renders_all_registry_entries
- test_quick_dispatches_to_backend
- test_unknown_command_first_time_emits_hint
- test_unknown_command_second_time_is_silent
- test_resume_with_arg_means_session_resume_without_means_unpause
- test_hotkey_tab_toggles_engine_pane
- test_hotkey_ctrl_p_dispatches_pause
- test_preflight_active_blocks_normal_keys
- test_preflight_q_emits_downgrade_action
- test_quit_with_active_goal_confirms

Используй StubBackend.

==================================================================
СКЕЛЕТ
==================================================================

src/cli/chat/commands/mod.rs:
    pub mod registry;
    pub mod parser;
    pub mod dispatch;
    pub mod backend;
    pub mod help;
    pub mod completions;

src/vis/shell/keymap.rs:
    pub enum Action {
        SendText(String),
        ToggleEnginePane(EnginePaneTarget),
        DispatchCommand(Command),
        StartInjectInline,
        ClearConversationView,
        ScrollConversation(ScrollDir),
        HistoryNav(HistoryDir),
        Quit,
        Cancel,
        PreflightAccept,
        PreflightExplain,
        PreflightDowngrade,
        PreflightCancel,
    }
    pub fn map_key_to_action(key: KeyEvent, ctx: &InputContext) -> Option<Action>;

==================================================================
SUCCESS CRITERIA
==================================================================

- cargo build --workspace зелено
- cargo test --test control_surface_test зелено ≥15
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status: только src/cli/chat/commands/**, src/vis/shell/keymap.rs, tests/control_surface_test.rs
- PR title: "feat(chat): control surface — slash commands, hotkeys, preflight keys (W5)"
- PR body: список не подключенных backend методов с указанием кто склеит на coordination day.

==================================================================
СТОП-ТРИГГЕРЫ
==================================================================

- src/vis/shell/ не существует (W1 не смерджен) — OK, создавай. PR body: "depends on W1 merging before this can be wired into actual key event loop".
- shlex отсутствует — пиши минимальный собственный quote-aware split.
- CommandBackend получается с 25+ методами и неуклюжий — разбей на несколько trait'ов (RouterBackend, GoalBackend, SessionBackend).

НЕ ПЕРЕСМАТРИВАЙ §8, §13, §14.

Начинай с разведки.
