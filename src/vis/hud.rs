use anyhow::Result;

/// Run the TUI HUD (requires --features tui)
#[cfg(feature = "tui")]
pub async fn run_tui() -> Result<()> {
    use ratatui::{
        backend::CrosstermBackend,
        crossterm::{
            event::{self, Event, KeyCode},
            terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
            ExecutableCommand,
        },
        Terminal,
    };
    use std::io::stdout;

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|frame| {
            let area = frame.area();
            let text = ratatui::widgets::Paragraph::new("omk HUD\n\nPress 'q' to quit")
                .block(
                    ratatui::widgets::Block::default()
                        .title("Oh My Kimi")
                        .borders(ratatui::widgets::Borders::ALL),
                );
            frame.render_widget(text, area);
        })?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    should_quit = true;
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

#[cfg(not(feature = "tui"))]
pub async fn run_tui() -> Result<()> {
    anyhow::bail!("TUI feature not enabled")
}
