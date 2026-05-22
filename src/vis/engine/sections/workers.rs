use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::engine::model::{PaneModel, WorkerStatus};
use crate::vis::engine::theme::Theme;

pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    if model.workers.is_empty() {
        return;
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        "workers:",
        Style::default().fg(theme.fg_emphasis()),
    )])];

    let mut workers: Vec<_> = model.workers.values().collect();
    workers.sort_by(|a, b| a.worker_id.cmp(&b.worker_id));
    for w in workers {
        let (sym, color) = match w.status {
            WorkerStatus::Running => ('●', theme.status_running()),
            WorkerStatus::Done => ('✓', theme.status_done()),
            WorkerStatus::Failed => ('✗', theme.status_failed()),
        };

        let elapsed = w.elapsed(model.now);
        let elapsed_str = format_duration(elapsed);

        let task = w.message.as_ref().unwrap_or(&w.task);

        lines.push(Line::from(vec![
            Span::styled(format!("{sym} "), Style::default().fg(color)),
            Span::styled(
                format!("{:10} ", &w.worker_id),
                Style::default().fg(theme.fg_normal()),
            ),
            Span::styled(
                format!("{:24} ", truncate(task, 24)),
                Style::default().fg(theme.fg_normal()),
            ),
            Span::styled(elapsed_str, Style::default().fg(theme.fg_muted())),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    let mins = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", mins, s)
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let idx = s
            .char_indices()
            .nth(max_chars - 1)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..idx])
    } else {
        s.to_string()
    }
}
