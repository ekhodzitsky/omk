use ratatui::{
    layout::Rect,
    style::Style,
    text::Text,
    widgets::{Block, Borders, Paragraph},
};

use crate::cli::chat::input::InputMode;
use crate::vis::shell::theme::Theme;

pub fn render_input(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    input_buffer: &str,
    input_mode: InputMode,
    theme: Theme,
) {
    let mode_label = match input_mode {
        InputMode::Text => "[text] ",
        InputMode::Command => "[cmd]  ",
    };

    let block = Block::default()
        .title("Input")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()));

    let text = format!("{}{}", mode_label, input_buffer);
    let para = Paragraph::new(Text::from(text)).block(block);
    frame.render_widget(para, area);
}
