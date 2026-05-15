use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

/// RAII guard that restores the terminal to a sane state — even on panic or
/// early `?` return from the run loop. Without this, a wire/event-stream
/// failure mid-render leaves the user's terminal in raw mode + alt-screen +
/// mouse-capture (no echo, no cursor, no working stty).
pub(super) struct RawModeGuard;

impl RawModeGuard {
    pub(super) fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        // Each restore step is best-effort: a failure here means we are
        // already on a fire and propagating it would just hide the original
        // error. Keep going so as much as possible gets reset.
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = execute!(std::io::stdout(), Show);
        let _ = disable_raw_mode();
    }
}
