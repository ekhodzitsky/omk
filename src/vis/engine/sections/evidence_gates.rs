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
    if model.evidence_gates.is_empty() {
        return;
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        "gates:",
        Style::default().fg(theme.fg_emphasis()),
    )])];

    let mut gates: Vec<_> = model.evidence_gates.values().collect();
    gates.sort_by(|a, b| a.gate.cmp(&b.gate));
    for g in gates {
        let (sym, color) = gate_symbol(&g.state, theme);
        lines.push(Line::from(vec![
            Span::styled(format!("{sym} "), Style::default().fg(color)),
            Span::styled(
                format!("{:12} ", &g.gate),
                Style::default().fg(theme.fg_normal()),
            ),
            Span::styled(&g.state, Style::default().fg(color)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn gate_symbol(state: &str, theme: Theme) -> (char, ratatui::style::Color) {
    match state {
        "passed" | "done" | "ready" => ('✓', theme.status_done()),
        "failed" | "error" => ('✗', theme.status_failed()),
        "running" | "gating" | "active" => ('●', theme.status_running()),
        _ => ('⧗', theme.status_pending()),
    }
}
