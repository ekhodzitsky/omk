use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::engine::model::PaneModel;
use crate::vis::engine::theme::Theme;

pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let line = Line::from(vec![
        Span::styled("cost: ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            format_tokens(model.cost.tokens_in),
            Style::default().fg(theme.fg_normal()),
        ),
        Span::styled(" in / ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            format_tokens(model.cost.tokens_out),
            Style::default().fg(theme.fg_normal()),
        ),
        Span::styled(" out  ·  ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            format!("${:.3}", model.cost.usd),
            Style::default().fg(theme.cost_meter()),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
