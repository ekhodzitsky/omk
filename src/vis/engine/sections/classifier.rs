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
    let mut lines = vec![];

    if let Some(ref latest) = model.classifier {
        lines.push(Line::from(vec![
            Span::styled("intent: ", Style::default().fg(theme.fg_muted())),
            Span::styled(
                format!("{:?}", latest.intent).to_lowercase(),
                Style::default()
                    .fg(theme.fg_emphasis())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  conf ", Style::default().fg(theme.fg_muted())),
            Span::styled(
                format!("{:.2}", latest.confidence),
                Style::default().fg(theme.fg_normal()),
            ),
            Span::styled("  ·  ", Style::default().fg(theme.fg_muted())),
            Span::styled(
                format!("{}ms", latest.latency_ms),
                Style::default().fg(theme.fg_normal()),
            ),
        ]));

        let reasoning = &latest.reasoning;
        let preview = if reasoning.chars().count() > 60 {
            format!(
                "{}…",
                &reasoning[..reasoning
                    .char_indices()
                    .nth(60)
                    .map(|(i, _)| i)
                    .unwrap_or(reasoning.len())]
            )
        } else {
            reasoning.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("reasoning: ", Style::default().fg(theme.fg_muted())),
            Span::styled(preview, Style::default().fg(theme.fg_normal())),
        ]));
    }

    if !model.recent_classifications.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "recent:",
            Style::default().fg(theme.fg_muted()),
        )]));
        for c in model.recent_classifications.iter().take(5) {
            let ts = c.ts.format("%H:%M:%S").to_string();
            let intent = format!("{:?}", c.intent).to_lowercase();
            let prefix = if c.reasoning.chars().count() > 20 {
                format!(
                    "{}…",
                    &c.reasoning[..c
                        .reasoning
                        .char_indices()
                        .nth(20)
                        .map(|(i, _)| i)
                        .unwrap_or(c.reasoning.len())]
                )
            } else {
                c.reasoning.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {}  ", ts), Style::default().fg(theme.fg_muted())),
                Span::styled(
                    format!("{:10}", intent),
                    Style::default().fg(theme.fg_normal()),
                ),
                Span::styled(
                    format!("\"{}\"", prefix),
                    Style::default().fg(theme.fg_muted()),
                ),
            ]));
        }
    }

    if lines.is_empty() {
        return;
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
