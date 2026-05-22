use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::engine::model::PaneModel;
use crate::vis::engine::theme::Theme;

pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let uptime = format_duration(model.session.uptime);
    let cost = format!("${:.3}", model.cost.usd);

    let line = Line::from(vec![
        Span::styled("session: ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            &model.session.id,
            Style::default()
                .fg(theme.fg_emphasis())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            &model.session.project_short,
            Style::default().fg(theme.fg_normal()),
        ),
        Span::styled("  ·  up ", Style::default().fg(theme.fg_muted())),
        Span::styled(uptime, Style::default().fg(theme.fg_normal())),
        Span::styled("  ·  ", Style::default().fg(theme.fg_muted())),
        Span::styled(cost, Style::default().fg(theme.cost_meter())),
    ]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, mins, s)
    } else {
        format!("{:02}:{:02}", mins, s)
    }
}
