use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::cli::chat::persistence::Message;
use crate::vis::shell::theme::Theme;

pub fn render_conversation(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    messages: &[Message],
    _scroll: usize,
    theme: Theme,
) {
    let lines: Vec<Line> = messages
        .iter()
        .map(|m| {
            let prefix = match m.role.as_str() {
                "user" => Span::styled("you: ", Style::default().fg(theme.user_color())),
                _ => Span::styled("omk: ", Style::default().fg(theme.assistant_color())),
            };
            Line::from(vec![prefix, Span::raw(m.text.clone())])
        })
        .collect();

    let block = Block::default()
        .title("Conversation")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()));

    let para = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(para, area);
}
