use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::bus::PlanNodeStatus;
use crate::vis::engine::model::PaneModel;
use crate::vis::engine::theme::Theme;

pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let Some(ref plan) = model.plan else {
        return;
    };

    let mut lines = vec![Line::from(vec![Span::styled(
        "plan:",
        Style::default()
            .fg(theme.fg_emphasis())
            .add_modifier(Modifier::BOLD),
    )])];

    for node in &plan.nodes {
        let (sym, color) = match node.status {
            PlanNodeStatus::Pending => ('□', theme.fg_muted()),
            PlanNodeStatus::Running => ('▸', theme.status_running()),
            PlanNodeStatus::Done => ('✓', theme.status_done()),
            PlanNodeStatus::Failed => ('✗', theme.status_failed()),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{sym} "), Style::default().fg(color)),
            Span::styled(&node.label, Style::default().fg(theme.fg_normal())),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}
