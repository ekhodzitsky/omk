use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::commands::backend::CommandBackend;
use super::commands::{parse_command, tab_complete, CommandDispatcher, CommandSessionState};
use super::input::{ChatEvent, InputHistory, InputMode, KeyCode, KeyEvent};
use super::persistence::{ConversationLog, SessionMeta};
pub mod dispatch;
pub mod state;
pub use state::{AppAction, PaneState, SessionState};

use self::state::{check_tab_hint, default_config_dir, default_state_dir};

/// Main application state machine for the chat shell.
pub struct App {
    pub session: SessionState,
    pub pane_state: PaneState,
    pub input_buffer: String,
    pub input_mode: InputMode,
    pub history: InputHistory,
    pub last_activity: Instant,
    pub conversation_scroll: usize,
    pub confirm_quit: bool,
    pub tab_hint_seen: bool,
    pub state_dir: PathBuf,
    pub config_dir: PathBuf,
    backend: Arc<dyn CommandBackend>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("session", &self.session)
            .field("pane_state", &self.pane_state)
            .field("input_buffer", &self.input_buffer)
            .field("input_mode", &self.input_mode)
            .field("history", &self.history)
            .field("last_activity", &self.last_activity)
            .field("conversation_scroll", &self.conversation_scroll)
            .field("confirm_quit", &self.confirm_quit)
            .field("tab_hint_seen", &self.tab_hint_seen)
            .field("state_dir", &self.state_dir)
            .field("config_dir", &self.config_dir)
            .finish_non_exhaustive()
    }
}

impl App {
    /// Create a new App with default state directories and the stub backend.
    pub fn new(project_root: String, session_id: String) -> Result<Self> {
        Self::new_with_backend(
            project_root,
            session_id,
            Arc::new(super::commands::StubBackend),
        )
    }

    /// Create a new App with an explicit backend.
    pub fn new_with_backend(
        project_root: String,
        session_id: String,
        backend: Arc<dyn CommandBackend>,
    ) -> Result<Self> {
        Self::with_dirs(
            default_state_dir(&session_id),
            default_config_dir(),
            project_root,
            session_id,
            backend,
        )
    }

    /// Create a new App with explicit directories (used by tests).
    pub fn with_dirs(
        state_dir: PathBuf,
        config_dir: PathBuf,
        project_root: String,
        session_id: String,
        backend: Arc<dyn CommandBackend>,
    ) -> Result<Self> {
        std::fs::create_dir_all(&state_dir)?;
        std::fs::create_dir_all(&config_dir)?;

        let meta_path = state_dir.join("meta.json");
        let meta = if meta_path.exists() {
            SessionMeta::load(&meta_path)?
        } else {
            let now = chrono::Utc::now();
            SessionMeta {
                session_id: session_id.clone(),
                started_at: now,
                project_root,
                last_activity: now,
                theme: "dark".to_string(),
                schema_version: 1,
            }
        };

        let conv_path = state_dir.join("conversation.jsonl");
        let conversation = ConversationLog::open(&conv_path)?;

        let history_path = state_dir.join("session-history.jsonl");
        let history = InputHistory::new(Some(history_path));

        // Create engine-events.jsonl (contract for W2/W3/W4) even if empty.
        let engine_path = state_dir.join("engine-events.jsonl");
        if !engine_path.exists() {
            std::fs::File::create(&engine_path)?;
        }

        let tab_hint_seen = check_tab_hint(&config_dir);

        let app = Self {
            session: SessionState { meta, conversation },
            pane_state: PaneState::Collapsed,
            input_buffer: String::new(),
            input_mode: InputMode::Text,
            history,
            last_activity: Instant::now(),
            conversation_scroll: 0,
            confirm_quit: false,
            tab_hint_seen,
            state_dir,
            config_dir,
            backend,
        };
        app.save_meta()?;
        Ok(app)
    }

    /// Process a single incoming event.
    pub fn handle_event(&mut self, ev: ChatEvent) -> AppAction {
        self.last_activity = Instant::now();
        match ev {
            ChatEvent::Key(key) => self.handle_key(key),
            ChatEvent::Tick => {
                self.check_idle();
                AppAction::Continue
            }
        }
    }

    /// Called on a timer to update time-dependent state.
    pub fn tick(&mut self) {
        self.check_idle();
    }

    fn check_idle(&mut self) {
        if self.pane_state != PaneState::Collapsed
            && self.last_activity.elapsed() > Duration::from_secs(300)
        {
            self.pane_state = PaneState::Collapsed;
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        if self.confirm_quit {
            return self.handle_confirm_quit(key);
        }

        // Global shortcuts (work in both Text and Command modes).
        match key.code {
            KeyCode::Char(c) if key.modifiers.control && c == 'l' => {
                self.conversation_scroll = 0;
                return AppAction::Redraw;
            }
            KeyCode::Char(c) if key.modifiers.control && (c == 'c' || c == 'd') => {
                self.confirm_quit = true;
                return AppAction::Redraw;
            }
            KeyCode::BackTab => {
                self.pane_state = PaneState::Collapsed;
                return AppAction::Redraw;
            }
            KeyCode::PageUp => {
                self.conversation_scroll = self.conversation_scroll.saturating_add(1);
                return AppAction::Redraw;
            }
            KeyCode::PageDown => {
                self.conversation_scroll = self.conversation_scroll.saturating_sub(1);
                return AppAction::Redraw;
            }
            _ => {}
        }

        if self.input_mode == InputMode::Command {
            return match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Text;
                    self.input_buffer.clear();
                    AppAction::Redraw
                }
                KeyCode::Tab => {
                    let completions = tab_complete(&self.input_buffer);
                    if completions.len() == 1 {
                        self.input_buffer = completions[0].clone();
                    }
                    AppAction::Redraw
                }
                KeyCode::Enter => {
                    let input = self.input_buffer.clone();
                    self.input_mode = InputMode::Text;
                    self.input_buffer.clear();
                    self.execute_command(&input)
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                    AppAction::Redraw
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                    if self.input_buffer.is_empty() {
                        self.input_mode = InputMode::Text;
                    }
                    AppAction::Redraw
                }
                _ => AppAction::Continue,
            };
        }

        match key.code {
            KeyCode::Char('/')
                if self.input_mode == InputMode::Text && self.input_buffer.is_empty() =>
            {
                self.input_mode = InputMode::Command;
                self.input_buffer.push('/');
                AppAction::Redraw
            }
            KeyCode::Tab => {
                self.record_tab_hint_seen();
                self.pane_state = match self.pane_state {
                    PaneState::Collapsed => PaneState::Expanded,
                    PaneState::Compact => PaneState::Expanded,
                    PaneState::Expanded => PaneState::Compact,
                };
                AppAction::Redraw
            }
            KeyCode::Up => {
                if let Some(text) = self.history.navigate_up() {
                    self.input_buffer = text.to_string();
                }
                AppAction::Redraw
            }
            KeyCode::Down => {
                if let Some(text) = self.history.navigate_down() {
                    self.input_buffer = text.to_string();
                } else {
                    self.input_buffer.clear();
                }
                AppAction::Redraw
            }
            KeyCode::Esc => AppAction::Redraw,
            KeyCode::Enter => {
                if key.modifiers.shift {
                    self.input_buffer.push('\n');
                    AppAction::Redraw
                } else {
                    self.submit_input()
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                AppAction::Redraw
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                AppAction::Redraw
            }
            _ => AppAction::Continue,
        }
    }

    fn handle_confirm_quit(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => AppAction::Quit,
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.confirm_quit = false;
                AppAction::Redraw
            }
            _ => AppAction::Continue,
        }
    }

    fn submit_input(&mut self) -> AppAction {
        let text = self.input_buffer.clone();
        self.input_buffer.clear();

        if text.trim().is_empty() {
            return AppAction::Redraw;
        }

        let _ = self.session.conversation.append_user(&text);
        self.history.push(text.clone());

        // W1 stub echo.
        let truncated = if text.len() > 60 {
            format!("{}...", &text[..60])
        } else {
            text.clone()
        };
        let escaped = truncated.replace('\n', "\\n");
        let stub = format!("[W1 stub] received \"{}\"", escaped);
        let _ = self.session.conversation.append_assistant(&stub);

        self.touch_meta();
        AppAction::Redraw
    }

    fn execute_command(&mut self, input: &str) -> AppAction {
        let dispatcher =
            CommandDispatcher::new(self.backend.clone(), Arc::new(CommandSessionState::new()));

        match parse_command(input) {
            Ok(cmd) => {
                let resp = match tokio::runtime::Handle::try_current() {
                    Ok(handle) => handle.block_on(dispatcher.dispatch(cmd)),
                    Err(_) => {
                        let _ = self
                            .session
                            .conversation
                            .append_assistant("Error: async runtime not available");
                        return AppAction::Redraw;
                    }
                };
                dispatch::apply_response(self, resp)
            }
            Err(_) => {
                let _ = self
                    .session
                    .conversation
                    .append_assistant("Invalid command syntax");
                AppAction::Redraw
            }
        }
    }

    fn touch_meta(&mut self) {
        self.session.meta.last_activity = chrono::Utc::now();
        let _ = self.save_meta();
    }

    fn save_meta(&self) -> Result<()> {
        let path = self.state_dir.join("meta.json");
        self.session.meta.save(&path)
    }

    fn record_tab_hint_seen(&mut self) {
        if self.tab_hint_seen {
            return;
        }
        self.tab_hint_seen = true;
        let path = self.config_dir.join("seen.json");
        let _ = std::fs::write(&path, r#"{"tab_hint":true}"#);
    }
}
