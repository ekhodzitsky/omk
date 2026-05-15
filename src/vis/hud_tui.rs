use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

/// RAII guard that restores the terminal to a sane state — even on panic or
/// early `?` return from the run loop. Without this, a wire/event-stream
/// failure mid-render leaves the user's terminal in raw mode + alt-screen +
/// mouse-capture (no echo, no cursor, no working stty).
struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        // Each restore step is best-effort: a failure here means we are
        // already on a fire and propagating it would just hide the original
        // error. Keep going so as much as possible gets reset.
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = execute!(std::io::stdout(), Show);
        let _ = disable_raw_mode();
    }
}
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame, Terminal,
};

use crate::runtime::events::EventKind;
use crate::runtime::state::{TaskStatus, TeamState};
use crate::runtime::watchdog::Watchdog;
use crate::vis::event_stream::EventStream;
use crate::vis::hud::{strip_ansi, HudState};

pub struct HudTui {
    hud_state: HudState,
    event_stream: EventStream,
    watchdog: Watchdog,
    state_dir: PathBuf,
    team_name: String,
    team_state: Option<TeamState>,
}

impl HudTui {
    pub fn new(team_name: &str, state_dir: PathBuf) -> Self {
        let events_path = state_dir.join("events.jsonl");
        let event_stream = EventStream::new(&events_path);
        let watchdog = Watchdog::new(crate::runtime::watchdog::WatchdogConfig {
            ..Default::default()
        });
        let run_id = team_name.to_string();
        let hud_state = HudState::new(team_name, &run_id);
        Self {
            hud_state,
            event_stream,
            watchdog,
            state_dir,
            team_name: team_name.to_string(),
            team_state: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Guard restores raw mode / alt screen / mouse capture on Drop, so a
        // panic or any `?` short-circuit inside run_loop no longer leaves the
        // user's terminal corrupted.
        let _guard = RawModeGuard::enter()?;
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;
        self.run_loop(&mut terminal).await
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let mut last_tick = std::time::Instant::now();
        let tick_rate = std::time::Duration::from_millis(1000);

        // Initial refresh
        self.hud_state
            .refresh(&mut self.event_stream, &self.watchdog, &self.state_dir)
            .await?;
        self.team_state = TeamState::load(&self.state_dir).await.ok();

        loop {
            terminal.draw(|f| self.draw(f))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| std::time::Duration::from_secs(0));

            if event::poll(timeout)? {
                if let CrosstermEvent::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('r') => {
                            self.hud_state
                                .refresh(&mut self.event_stream, &self.watchdog, &self.state_dir)
                                .await?;
                            self.team_state = TeamState::load(&self.state_dir).await.ok();
                        }
                        _ => {}
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.hud_state
                    .refresh(&mut self.event_stream, &self.watchdog, &self.state_dir)
                    .await?;
                self.team_state = TeamState::load(&self.state_dir).await.ok();
                last_tick = std::time::Instant::now();
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Length(1),
            ])
            .split(area);

        self.draw_header(frame, chunks[0]);
        self.draw_workers(frame, chunks[1]);
        self.draw_tasks(frame, chunks[2]);
        self.draw_events(frame, chunks[3]);
        self.draw_footer(frame, chunks[4]);
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let runtime = self
            .hud_state
            .last_update
            .signed_duration_since(self.hud_state.start_time);
        let runtime_str = format!(
            "{}:{:02}:{:02}",
            runtime.num_hours(),
            runtime.num_minutes().rem_euclid(60),
            runtime.num_seconds().rem_euclid(60)
        );

        let (status_label, status_color) = if self
            .hud_state
            .events
            .iter()
            .any(|e| matches!(e.kind, EventKind::RunCompleted))
        {
            ("Completed", Color::Green)
        } else if self
            .hud_state
            .events
            .iter()
            .any(|e| matches!(e.kind, EventKind::RunFailed | EventKind::ManualInterrupt))
        {
            ("Failed", Color::Red)
        } else {
            ("Running", Color::Yellow)
        };

        let header_text = Text::from(vec![
            Line::from(vec![
                Span::styled(
                    format!("OMK HUD — {} ", strip_ansi(&self.team_name)),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("run: {} ", strip_ansi(&self.hud_state.run_id)),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("runtime: {} ", runtime_str),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(status_label, Style::default().fg(status_color)),
            ]),
            Line::from(vec![Span::styled(
                format!(
                    "Tasks: {} total | {} completed | {} running | {} pending | {} failed",
                    self.hud_state.task_summary.total,
                    self.hud_state.task_summary.completed,
                    self.hud_state.task_summary.running,
                    self.hud_state.task_summary.pending,
                    self.hud_state.task_summary.failed,
                ),
                Style::default().fg(Color::Gray),
            )]),
        ]);

        let header = Paragraph::new(header_text).block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(header, area);
    }

    fn draw_workers(&self, frame: &mut Frame, area: Rect) {
        let displays = self.hud_state.worker_displays();

        let header = Row::new(vec!["Worker", "Status", "Task", "HB Age", "Retry", "Gates"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(0);

        let rows: Vec<Row> = displays
            .iter()
            .map(|d| {
                let (status_color, status_text) = match d.status.as_str() {
                    "Healthy" | "Ready" | "Busy" => (Color::Green, d.status.as_str()),
                    "Stalled" => (Color::Yellow, "Stalled"),
                    "Dead" => (Color::Red, "Dead"),
                    _ => (Color::Gray, "Unknown"),
                };

                let age_str = if d.heartbeat_age_secs >= 0 {
                    format!("{}s", d.heartbeat_age_secs)
                } else {
                    "N/A".to_string()
                };

                let task_str = d
                    .current_task_id
                    .as_deref()
                    .map(strip_ansi)
                    .unwrap_or_else(|| "-".to_string());

                Row::new(vec![
                    Cell::from(strip_ansi(&d.name)),
                    Cell::from(Span::styled(status_text, Style::default().fg(status_color))),
                    Cell::from(task_str),
                    Cell::from(age_str),
                    Cell::from(d.retry_count.to_string()),
                    Cell::from(strip_ansi(&d.gate_status)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(15),
                Constraint::Percentage(25),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(16),
            ],
        )
        .header(header)
        .block(Block::default().title("Workers").borders(Borders::ALL));

        frame.render_widget(table, area);
    }

    fn draw_tasks(&self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Task ID", "Status", "Worker", "Priority"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = if let Some(ref state) = self.team_state {
            state
                .tasks
                .iter()
                .map(|t| {
                    let (status_color, status_text) = match t.status {
                        TaskStatus::Pending => (Color::Gray, "Pending"),
                        TaskStatus::InProgress => (Color::Yellow, "Running"),
                        TaskStatus::Done => (Color::Green, "Completed"),
                        TaskStatus::Failed => (Color::Red, "Failed"),
                    };

                    let worker_str = t
                        .assigned_to
                        .as_deref()
                        .map(strip_ansi)
                        .unwrap_or_default();

                    Row::new(vec![
                        Cell::from(strip_ansi(&t.id)),
                        Cell::from(Span::styled(status_text, Style::default().fg(status_color))),
                        Cell::from(worker_str),
                        Cell::from("-"),
                    ])
                })
                .collect()
        } else {
            vec![Row::new(vec![
                Cell::from("No tasks loaded"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])]
        };

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(35),
                Constraint::Percentage(20),
                Constraint::Percentage(25),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .block(Block::default().title("Tasks").borders(Borders::ALL));

        frame.render_widget(table, area);
    }

    fn draw_events(&self, frame: &mut Frame, area: Rect) {
        let event_lines: Vec<Line> = self
            .hud_state
            .events
            .iter()
            .rev()
            .take(10)
            .map(|e| {
                let ts = e.ts.format("%H:%M:%S").to_string();
                let actor = e
                    .actor
                    .as_deref()
                    .map(strip_ansi)
                    .unwrap_or_else(|| "-".to_string());
                let kind = format!("{:?}", e.kind);
                Line::from(vec![
                    Span::styled(format!("{} ", ts), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("[{}] ", actor), Style::default().fg(Color::Cyan)),
                    Span::raw(kind),
                ])
            })
            .collect();

        let events_widget = Paragraph::new(Text::from(event_lines))
            .block(Block::default().title("Events").borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(events_widget, area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let footer = Paragraph::new(Span::styled(
            " q=quit | r=refresh ",
            Style::default().fg(Color::Gray),
        ));
        frame.render_widget(footer, area);
    }

    #[allow(dead_code)]
    fn worker_task_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for event in &self.hud_state.events {
            if let Some(ref payload) = event.payload {
                let worker_id = match event.actor.clone() {
                    Some(id) => id,
                    None => continue,
                };

                if let Some(task_id) = payload.get("task_id").and_then(|v| v.as_str()) {
                    match event.kind {
                        EventKind::TaskClaimed | EventKind::TaskStarted => {
                            map.insert(worker_id, task_id.to_string());
                        }
                        EventKind::TaskCompleted | EventKind::TaskFailed => {
                            map.remove(&worker_id);
                        }
                        EventKind::WorkerHeartbeat => {
                            map.entry(worker_id).or_insert_with(|| task_id.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn hud_tui_draw_does_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let hud = HudTui::new("test-team", tmp.path().to_path_buf());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| hud.draw(f)).unwrap();
    }

    #[test]
    fn hud_tui_worker_colors() {
        let tmp = tempfile::tempdir().unwrap();
        let mut hud = HudTui::new("test-team", tmp.path().to_path_buf());

        use crate::runtime::watchdog::{HealthStatus, WorkerHealth};
        use chrono::Utc;

        hud.hud_state.workers = vec![
            WorkerHealth {
                worker_id: "w1".to_string(),
                status: HealthStatus::Healthy,
                last_heartbeat: Some(Utc::now()),
                heartbeat_content: None,
                inbox_count: 0,
                outbox_count: 0,
                message: "ok".to_string(),
            },
            WorkerHealth {
                worker_id: "w2".to_string(),
                status: HealthStatus::Stalled,
                last_heartbeat: Some(Utc::now()),
                heartbeat_content: None,
                inbox_count: 0,
                outbox_count: 0,
                message: "stalled".to_string(),
            },
            WorkerHealth {
                worker_id: "w3".to_string(),
                status: HealthStatus::Dead,
                last_heartbeat: None,
                heartbeat_content: None,
                inbox_count: 0,
                outbox_count: 0,
                message: "dead".to_string(),
            },
        ];

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| hud.draw(f)).unwrap();
        // If we got here without panic, color mapping is valid.
    }
}
