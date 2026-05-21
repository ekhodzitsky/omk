use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Colour theme for the chat shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn fg_normal(&self) -> Color {
        match self {
            Theme::Dark => Color::White,
            Theme::Light => Color::Black,
        }
    }

    pub fn bg(&self) -> Color {
        match self {
            Theme::Dark => Color::Black,
            Theme::Light => Color::White,
        }
    }

    pub fn engine_status(&self) -> Color {
        match self {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Blue,
        }
    }

    pub fn border(&self) -> Color {
        match self {
            Theme::Dark => Color::Gray,
            Theme::Light => Color::DarkGray,
        }
    }

    pub fn highlight(&self) -> Color {
        match self {
            Theme::Dark => Color::Yellow,
            Theme::Light => Color::Magenta,
        }
    }

    pub fn user_color(&self) -> Color {
        match self {
            Theme::Dark => Color::Yellow,
            Theme::Light => Color::Blue,
        }
    }

    pub fn assistant_color(&self) -> Color {
        match self {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Green,
        }
    }
}
