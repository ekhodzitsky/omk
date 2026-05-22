use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::cli::chat::commands::parser::Command;

/// High-level action produced by the keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // -- Text input --
    SendText(String),
    InsertChar(char),
    DeleteCharBackward,
    DeleteCharForward,

    // -- Pane control --
    ToggleEnginePane(EnginePaneTarget),

    // -- Command mode entry / exit --
    EnterCommandMode,
    ExitCommandMode,

    // -- Parsed command dispatch --
    DispatchCommand(Command),

    // -- Semantic shortcuts to slash commands --
    DispatchCommandByName(&'static str, Vec<String>),

    // -- Inline UI --
    StartInjectInline,
    ClearConversationView,

    // -- Scroll / navigation --
    ScrollConversation(ScrollDir),
    HistoryNav(HistoryDir),

    // -- Lifecycle --
    Quit,
    ConfirmAction(ConfirmTarget),
    Cancel,

    // -- Preflight dialog (only when preflight is showing) --
    PreflightAccept,
    PreflightExplain,
    PreflightDowngrade,
    PreflightCancel,

    // -- No-op --
    Ignore,
}

/// Target state for the engine pane toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnginePaneTarget {
    Toggle,
    Collapse,
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDir {
    Up,
    Down,
    PageUp,
    PageDown,
}

/// History navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryDir {
    Prev,
    Next,
}

/// What the user is confirming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmTarget {
    Quit,
    Cancel,
    Reject,
}

/// Input modality used by the keymap.
///
/// W1 defines a smaller `InputMode` (Text / Command).  This enum is
/// extended with the additional states required by the control surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Text,
    Command,
    PreflightActive,
    InlineDialog,
}

/// Context needed to resolve a key event to an action.
#[derive(Debug, Clone)]
pub struct InputContext {
    pub mode: InputMode,
    pub preflight_active: bool,
    pub has_active_large_goal: bool,
}

/// Map a crossterm key event to a high-level action.
///
/// The caller (W1's `App::handle_event`) is responsible for translating
/// its portable `KeyEvent` into a crossterm `KeyEvent` before calling
/// this function — this will be done by the orchestrator on coordination
/// day.
pub fn map_key_to_action(key: KeyEvent, ctx: &InputContext) -> Action {
    // 1. PreflightActive absorbs almost everything.
    if ctx.preflight_active {
        return match key.code {
            KeyCode::Enter => Action::PreflightAccept,
            KeyCode::Char('e') | KeyCode::Char('E') => Action::PreflightExplain,
            KeyCode::Char('q') | KeyCode::Char('Q') => Action::PreflightDowngrade,
            KeyCode::Esc => Action::PreflightCancel,
            _ => Action::Ignore,
        };
    }

    // 2. Universal hotkeys (any mode except PreflightActive).
    match key.code {
        KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
            return Action::ToggleEnginePane(EnginePaneTarget::Toggle);
        }
        KeyCode::BackTab if key.modifiers == KeyModifiers::SHIFT => {
            return Action::ToggleEnginePane(EnginePaneTarget::Collapse);
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::DispatchCommandByName("pause", vec![]);
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::DispatchCommandByName("resume", vec![]);
        }
        KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::DispatchCommandByName("cancel", vec![]);
        }
        KeyCode::Char('i') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::StartInjectInline;
        }
        KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::DispatchCommandByName("approve", vec![]);
        }
        KeyCode::Char('j') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::DispatchCommandByName("reject", vec![]);
        }
        KeyCode::Char('l') if key.modifiers == KeyModifiers::CONTROL => {
            return Action::ClearConversationView;
        }
        KeyCode::Char('d') | KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
            if ctx.has_active_large_goal {
                return Action::ConfirmAction(ConfirmTarget::Quit);
            }
            return Action::Quit;
        }
        KeyCode::PageUp => return Action::ScrollConversation(ScrollDir::PageUp),
        KeyCode::PageDown => return Action::ScrollConversation(ScrollDir::PageDown),
        _ => {}
    }

    // 3. Mode-specific routing.
    match ctx.mode {
        InputMode::Text => match key.code {
            KeyCode::Enter => Action::SendText(String::new()),
            KeyCode::Up => Action::HistoryNav(HistoryDir::Prev),
            KeyCode::Down => Action::HistoryNav(HistoryDir::Next),
            KeyCode::Char(c) => Action::InsertChar(c),
            KeyCode::Backspace => Action::DeleteCharBackward,
            KeyCode::Delete => Action::DeleteCharForward,
            _ => Action::Ignore,
        },
        InputMode::Command => match key.code {
            KeyCode::Enter => Action::DispatchCommand(Command {
                name: String::new(),
                args: vec![],
                raw_args: String::new(),
            }),
            KeyCode::Esc => Action::ExitCommandMode,
            KeyCode::Char(c) => Action::InsertChar(c),
            KeyCode::Backspace => Action::DeleteCharBackward,
            KeyCode::Delete => Action::DeleteCharForward,
            _ => Action::Ignore,
        },
        InputMode::PreflightActive => {
            // Defensive — unreachable because of the early return above.
            Action::Ignore
        }
        InputMode::InlineDialog => match key.code {
            KeyCode::Esc => Action::Cancel,
            KeyCode::Enter => Action::SendText(String::new()),
            KeyCode::Char(c) => Action::InsertChar(c),
            KeyCode::Backspace => Action::DeleteCharBackward,
            _ => Action::Ignore,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn ctx(mode: InputMode, preflight: bool, active_goal: bool) -> InputContext {
        InputContext {
            mode,
            preflight_active: preflight,
            has_active_large_goal: active_goal,
        }
    }

    #[test]
    fn test_hotkey_tab_toggles_engine_pane() {
        let action = map_key_to_action(
            key(KeyCode::Tab, KeyModifiers::NONE),
            &ctx(InputMode::Text, false, false),
        );
        assert_eq!(action, Action::ToggleEnginePane(EnginePaneTarget::Toggle));
    }

    #[test]
    fn test_hotkey_ctrl_p_dispatches_pause() {
        let action = map_key_to_action(
            key(KeyCode::Char('p'), KeyModifiers::CONTROL),
            &ctx(InputMode::Text, false, false),
        );
        assert_eq!(action, Action::DispatchCommandByName("pause", vec![]));
    }

    #[test]
    fn test_preflight_active_blocks_normal_keys() {
        let c = ctx(InputMode::Text, true, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('a'), KeyModifiers::NONE), &c),
            Action::Ignore
        );
        assert_eq!(
            map_key_to_action(key(KeyCode::Tab, KeyModifiers::NONE), &c),
            Action::Ignore
        );
    }

    #[test]
    fn test_preflight_q_emits_downgrade_action() {
        let c = ctx(InputMode::Text, true, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('q'), KeyModifiers::NONE), &c),
            Action::PreflightDowngrade
        );
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('Q'), KeyModifiers::NONE), &c),
            Action::PreflightDowngrade
        );
    }

    #[test]
    fn test_quit_with_active_goal_emits_confirm() {
        let c = ctx(InputMode::Text, false, true);
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('d'), KeyModifiers::CONTROL), &c),
            Action::ConfirmAction(ConfirmTarget::Quit)
        );
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('c'), KeyModifiers::CONTROL), &c),
            Action::ConfirmAction(ConfirmTarget::Quit)
        );
    }

    #[test]
    fn test_quit_without_active_goal_emits_direct_quit() {
        let c = ctx(InputMode::Text, false, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('d'), KeyModifiers::CONTROL), &c),
            Action::Quit
        );
    }

    #[test]
    fn test_esc_exits_command_mode() {
        let c = ctx(InputMode::Command, false, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::Esc, KeyModifiers::NONE), &c),
            Action::ExitCommandMode
        );
    }

    #[test]
    fn test_enter_in_command_mode_dispatches_command() {
        let c = ctx(InputMode::Command, false, false);
        let action = map_key_to_action(key(KeyCode::Enter, KeyModifiers::NONE), &c);
        assert!(matches!(action, Action::DispatchCommand(_)));
    }

    #[test]
    fn test_page_up_scrolls_conversation() {
        let c = ctx(InputMode::Text, false, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::PageUp, KeyModifiers::NONE), &c),
            Action::ScrollConversation(ScrollDir::PageUp)
        );
    }

    #[test]
    fn test_up_down_navigate_history() {
        let c = ctx(InputMode::Text, false, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::Up, KeyModifiers::NONE), &c),
            Action::HistoryNav(HistoryDir::Prev)
        );
        assert_eq!(
            map_key_to_action(key(KeyCode::Down, KeyModifiers::NONE), &c),
            Action::HistoryNav(HistoryDir::Next)
        );
    }

    #[test]
    fn test_shift_tab_collapses_pane() {
        let c = ctx(InputMode::Text, false, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::BackTab, KeyModifiers::SHIFT), &c),
            Action::ToggleEnginePane(EnginePaneTarget::Collapse)
        );
    }

    #[test]
    fn test_text_mode_typing_inserts_char() {
        let c = ctx(InputMode::Text, false, false);
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('x'), KeyModifiers::NONE), &c),
            Action::InsertChar('x')
        );
    }
}
