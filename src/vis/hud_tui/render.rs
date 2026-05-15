use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::runtime::events::EventKind;
use crate::runtime::state::TaskStatus;
use crate::vis::hud::strip_ansi;
use crate::vis::hud_tui::HudTui;

impl HudTui {
    pub(super) fn draw(&self, frame: &mut Frame) {
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

                    let worker_str = t.assigned_to.as_deref().map(strip_ansi).unwrap_or_default();

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
}
