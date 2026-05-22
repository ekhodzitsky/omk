use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::engine::blocks::truncate_text;
use crate::vis::engine::model::PaneModel;
use crate::vis::engine::theme::Theme;

/// Render the "Escalations" section.
///
/// Returns silently when the section is empty so the caller does not
/// consume layout space (UNIFIED_CHAT spec §7.6 — empty sections hide).
pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    if model.escalations.is_empty() {
        return;
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        "Escalations",
        Style::default().fg(theme.fg_emphasis()),
    )])];

    // Show newest first, cap at 5 visible lines so the pane does not explode.
    for esc in model.escalations.iter().rev().take(5) {
        let ts = esc.ts.format("%H:%M:%S").to_string();
        let glyph = esc.kind.glyph();
        let intent_str = esc.intent.map(|i| format!("{:?}", i).to_lowercase() + " ");
        let summary = truncate_text(&esc.summary, 40);

        let mut parts = vec![
            Span::styled(format!("{glyph} "), Style::default().fg(theme.fg_normal())),
            Span::styled(format!("{ts} "), Style::default().fg(theme.fg_muted())),
        ];

        if let Some(intent) = intent_str {
            parts.push(Span::styled(intent, Style::default().fg(theme.fg_normal())));
        }

        parts.push(Span::styled(
            summary,
            Style::default().fg(theme.fg_normal()),
        ));

        lines.push(Line::from(parts));
    }

    frame.render_widget(Paragraph::new(lines), area);
}
