use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::cli::chat::app::PaneState;
use crate::vis::shell::theme::Theme;

pub fn render_engine(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    pane_state: PaneState,
    session_id: &str,
    show_hint: bool,
    theme: Theme,
) {
    match pane_state {
        PaneState::Collapsed => {
            let hint = if show_hint {
                "[Press Tab to see what's happening under the hood] "
            } else {
                ""
            };
            let text = Text::from(Line::from(vec![
                Span::raw("[engine] session: "),
                Span::styled(session_id, Style::default().fg(theme.engine_status())),
                Span::raw(" · idle · cost: $0.00 · Tab to expand "),
                Span::styled(hint, Style::default().fg(Color::Gray)),
            ]));
            let para = Paragraph::new(text);
            frame.render_widget(para, area);
        }
        PaneState::Compact => {
            let block = Block::default()
                .title("Engine")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()));
            let content = if show_hint {
                "[Press Tab to see what's happening under the hood]\nno events yet"
            } else {
                "no events yet"
            };
            let para = Paragraph::new(content).block(block);
            frame.render_widget(para, area);
        }
        PaneState::Expanded => {
            let block = Block::default()
                .title("Engine")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()));
            let content = if show_hint {
                "[Press Tab to see what's happening under the hood]\nno events yet"
            } else {
                "no events yet"
            };
            let para = Paragraph::new(content).block(block);
            frame.render_widget(para, area);
        }
    }
}
