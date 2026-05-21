use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::cli::chat::app::PaneState;

/// Computes the conversation and engine pane areas.
#[derive(Debug)]
pub struct ShellLayout;

impl ShellLayout {
    pub fn compute(area: Rect, pane_state: PaneState) -> (Rect, Rect) {
        let engine_width = Self::engine_width(area.width);

        let constraints = match pane_state {
            PaneState::Collapsed => {
                vec![
                    Constraint::Length(area.width.saturating_sub(1)),
                    Constraint::Length(1),
                ]
            }
            _ => {
                vec![
                    Constraint::Length(area.width.saturating_sub(engine_width)),
                    Constraint::Length(engine_width),
                ]
            }
        };

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        (chunks[0], chunks[1])
    }

    fn engine_width(total: u16) -> u16 {
        let percent = (total as f32 * 0.4) as u16;
        percent.max(50).min(total.saturating_sub(20))
    }
}
