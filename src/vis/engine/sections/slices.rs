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
    if model.slices.is_empty() {
        return;
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        "slices:",
        Style::default().fg(theme.fg_emphasis()),
    )])];

    for s in &model.slices {
        let wt = s.worktree.display().to_string();
        let pr = s
            .pr_url
            .as_ref()
            .map(|u| {
                if let Some(num) = u.rsplit('/').next().and_then(|n| n.parse::<u32>().ok()) {
                    format!("PR #{num}")
                } else {
                    u.clone()
                }
            })
            .unwrap_or_else(|| "no PR".into());

        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", &s.slice_id),
                Style::default().fg(theme.fg_normal()),
            ),
            Span::styled("· ", Style::default().fg(theme.fg_muted())),
            Span::styled(wt, Style::default().fg(theme.fg_normal())),
            Span::styled(" · ", Style::default().fg(theme.fg_muted())),
            Span::styled(pr, Style::default().fg(theme.fg_emphasis())),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}
