use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::engine::model::PaneModel;
use crate::vis::engine::theme::Theme;

pub fn render(_model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let line = Line::from(vec![Span::styled(
        "[P]ause  [I]nject  [A]pprove  [R]eject  [/] cmd",
        Style::default().fg(theme.fg_muted()),
    )]);
    frame.render_widget(Paragraph::new(line), area);
}
