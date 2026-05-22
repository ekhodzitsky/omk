use ratatui::style::Color;

/// Color-blind safe palette.  All color signals are duplicated with symbols
/// elsewhere, so the UI remains usable in monochrome terminals.
#[derive(Debug, Clone, Copy, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn bg(self) -> Color {
        match self {
            Theme::Dark => Color::Black,
            Theme::Light => Color::White,
        }
    }

    pub fn fg_normal(self) -> Color {
        match self {
            Theme::Dark => Color::White,
            Theme::Light => Color::Black,
        }
    }

    pub fn fg_muted(self) -> Color {
        match self {
            Theme::Dark => Color::Gray,
            Theme::Light => Color::DarkGray,
        }
    }

    pub fn fg_emphasis(self) -> Color {
        match self {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Blue,
        }
    }

    pub fn status_running(self) -> Color {
        Color::Yellow
    }

    pub fn status_done(self) -> Color {
        Color::Green
    }

    pub fn status_failed(self) -> Color {
        Color::Red
    }

    pub fn status_pending(self) -> Color {
        Color::DarkGray
    }

    pub fn cost_meter(self) -> Color {
        match self {
            Theme::Dark => Color::Magenta,
            Theme::Light => Color::Rgb(0x8B, 0x00, 0x8B),
        }
    }
}
