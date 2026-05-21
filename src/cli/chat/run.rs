use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;

use super::app::{App, AppAction, PaneState};
use super::input::{ChatEvent, KeyCode, KeyEvent, KeyModifiers};
use super::persistence::SessionMeta;
use super::session_id;

/// CLI arguments for `omk` when invoked without a subcommand.
#[derive(Args, Debug, Clone)]
pub struct ChatArgs {
    /// Resume a specific session by id.
    #[arg(long)]
    pub session: Option<String>,
    /// Start a fresh session, ignore existing.
    #[arg(long)]
    pub new: bool,
}

#[cfg(feature = "tui")]
pub fn run_chat(args: ChatArgs) -> Result<()> {
    use crossterm::{
        event::{
            self, Event as CrosstermEvent, KeyCode as CKeyCode, KeyModifiers as CKeyModifiers,
        },
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};

    // Setup terminal.
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("enter alt screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    // Resolve session.
    let project_root = resolve_project_root();
    let session_id = resolve_session_id(&args, &project_root)?;
    let state_dir = default_state_dir(&session_id);

    let mut app = App::with_dirs(state_dir, default_config_dir(), project_root, session_id)
        .context("build app")?;

    let tick_rate = Duration::from_millis(100);
    let result = loop {
        if let Err(e) = terminal.draw(|f| draw(f, &app)) {
            break Err(e.into());
        }

        let ev = match event::poll(tick_rate) {
            Ok(true) => match event::read() {
                Ok(CrosstermEvent::Key(key)) => {
                    let code = match key.code {
                        CKeyCode::Char(c) => KeyCode::Char(c),
                        CKeyCode::Enter => KeyCode::Enter,
                        CKeyCode::Tab => KeyCode::Tab,
                        CKeyCode::BackTab => KeyCode::BackTab,
                        CKeyCode::Up => KeyCode::Up,
                        CKeyCode::Down => KeyCode::Down,
                        CKeyCode::PageUp => KeyCode::PageUp,
                        CKeyCode::PageDown => KeyCode::PageDown,
                        CKeyCode::Esc => KeyCode::Esc,
                        CKeyCode::Backspace => KeyCode::Backspace,
                        _ => continue,
                    };
                    let modifiers = KeyModifiers {
                        shift: key.modifiers.contains(CKeyModifiers::SHIFT),
                        control: key.modifiers.contains(CKeyModifiers::CONTROL),
                        alt: key.modifiers.contains(CKeyModifiers::ALT),
                    };
                    ChatEvent::Key(KeyEvent { code, modifiers })
                }
                _ => continue,
            },
            Ok(false) => {
                app.tick();
                continue;
            }
            Err(e) => break Err(e.into()),
        };

        if app.handle_event(ev) == AppAction::Quit {
            break Ok(());
        }
    };

    // Teardown.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

#[cfg(not(feature = "tui"))]
pub fn run_chat(_args: ChatArgs) -> Result<()> {
    anyhow::bail!("tui feature not enabled")
}

#[cfg(feature = "tui")]
fn draw(f: &mut ratatui::Frame<'_>, app: &App) {
    use super::input::InputMode;
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        style::{Color, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Paragraph, Wrap},
    };

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(f.area());

    let conv_area = main_chunks[0];
    let engine_area = main_chunks[1];

    // Split conversation into display + input.
    let conv_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(conv_area);

    let display_area = conv_chunks[0];
    let input_area = conv_chunks[1];

    // Conversation display.
    let messages = app.session.conversation.read_all().unwrap_or_default();
    let text_lines: Vec<Line> = messages
        .iter()
        .map(|m| {
            let prefix = match m.role.as_str() {
                "user" => Span::styled("you: ", Style::default().fg(Color::Yellow)),
                _ => Span::styled("omk: ", Style::default().fg(Color::Cyan)),
            };
            Line::from(vec![prefix, Span::raw(m.text.clone())])
        })
        .collect();
    let conv = Paragraph::new(Text::from(text_lines))
        .block(Block::default().title("Conversation").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(conv, display_area);

    // Input box.
    let mode_label = match app.input_mode {
        InputMode::Text => "[text] ",
        InputMode::Command => "[cmd]  ",
    };
    let input_text = format!("{}{}", mode_label, app.input_buffer);
    let input =
        Paragraph::new(input_text).block(Block::default().title("Input").borders(Borders::ALL));
    f.render_widget(input, input_area);

    // Engine pane.
    match app.pane_state {
        PaneState::Collapsed => {
            let hint = if app.tab_hint_seen {
                ""
            } else {
                "[Press Tab to see what's happening under the hood]"
            };
            let status = format!(
                "[engine] session: {} · idle · cost: $0.00 · Tab to expand {}",
                app.session.meta.session_id, hint
            );
            let engine = Paragraph::new(status)
                .block(Block::default().title("Engine").borders(Borders::NONE));
            f.render_widget(engine, engine_area);
        }
        _ => {
            let block = Block::default().title("Engine").borders(Borders::ALL);
            let content = if app.tab_hint_seen {
                "no events yet"
            } else {
                "[Press Tab to see what's happening under the hood]\nno events yet"
            };
            let engine = Paragraph::new(content).block(block);
            f.render_widget(engine, engine_area);
        }
    }
}

fn resolve_project_root() -> String {
    use std::process::Command;
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
    }
}

fn resolve_session_id(args: &ChatArgs, project_root: &str) -> Result<String> {
    if args.new {
        return Ok(session_id::new_session_id());
    }
    if let Some(ref sid) = args.session {
        session_id::parse_session_id(sid).context("invalid session id")?;
        return Ok(sid.clone());
    }

    let sessions_dir = home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("state")
        .join("omk")
        .join("sessions");

    if !sessions_dir.exists() {
        return Ok(session_id::new_session_id());
    }

    let mut latest: Option<(std::fs::DirEntry, std::time::SystemTime)> = None;
    for entry in std::fs::read_dir(&sessions_dir)? {
        let entry = entry?;
        let meta_path = entry.path().join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        if let Ok(contents) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<SessionMeta>(&contents) {
                if meta.project_root == project_root {
                    if let Ok(m) = entry.metadata() {
                        if let Ok(modified) = m.modified() {
                            if latest.as_ref().map_or(true, |(_, t)| *t < modified) {
                                latest = Some((entry, modified));
                            }
                        }
                    }
                }
            }
        }
    }

    match latest {
        Some((entry, _)) => Ok(entry.file_name().to_string_lossy().to_string()),
        None => Ok(session_id::new_session_id()),
    }
}

fn default_state_dir(session_id: &str) -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("state")
        .join("omk")
        .join("sessions")
        .join(session_id)
}

fn default_config_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("omk")
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
