use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::bus::ActiveMode;
use crate::vis::engine::model::PaneModel;
use crate::vis::engine::state::PaneState;
use crate::vis::engine::theme::Theme;

mod sections {
    pub use super::super::sections::active_mode;
    pub use super::super::sections::classifier;
    pub use super::super::sections::cost;
    pub use super::super::sections::escalations;
    pub use super::super::sections::evidence_gates;
    pub use super::super::sections::footer;
    pub use super::super::sections::header;
    pub use super::super::sections::plan;
    pub use super::super::sections::slices;
    pub use super::super::sections::workers;
}

/// Render the engine pane into the given `area` of `frame`.
pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: &Theme) {
    match model.state {
        PaneState::Collapsed => render_collapsed(model, frame, area, *theme),
        PaneState::Compact => render_compact(model, frame, area, *theme),
        PaneState::Expanded => render_expanded(model, frame, area, *theme),
    }
}

fn render_collapsed(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let mut parts = vec![
        Span::styled("[engine] ", Style::default().fg(theme.fg_emphasis())),
        Span::styled(&model.session.id, Style::default().fg(theme.fg_normal())),
        Span::styled(" · ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            active_mode_word(model),
            Style::default().fg(theme.fg_normal()),
        ),
    ];

    if !model.has_failed_gate() && !model.workers.is_empty() {
        let active = model.active_worker_count();
        let total = model.total_worker_count();
        parts.push(Span::styled(" · ", Style::default().fg(theme.fg_muted())));
        parts.push(Span::styled(
            format!("workers {active}/{total}"),
            Style::default().fg(theme.fg_normal()),
        ));
    }

    if !model.escalations.is_empty() {
        parts.push(Span::styled(" · ", Style::default().fg(theme.fg_muted())));
        parts.push(Span::styled(
            format!("{} esc", model.escalations.len()),
            Style::default().fg(theme.fg_normal()),
        ));
    }

    parts.push(Span::styled(" · ", Style::default().fg(theme.fg_muted())));
    parts.push(Span::styled(
        format!("${:.2}", model.cost.usd),
        Style::default().fg(theme.cost_meter()),
    ));

    let suffix = if model.has_failed_gate() {
        "Tab to inspect"
    } else if !model.workers.is_empty() {
        "Tab"
    } else {
        "Tab to expand"
    };
    parts.push(Span::styled(" · ", Style::default().fg(theme.fg_muted())));
    parts.push(Span::styled(
        suffix,
        Style::default()
            .fg(theme.fg_muted())
            .add_modifier(Modifier::DIM),
    ));

    let line = Line::from(parts);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_compact(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let mut constraints = vec![Constraint::Length(1)]; // header

    let show_classifier = model.classifier.is_some();
    if show_classifier {
        constraints.push(Constraint::Min(0));
    }

    constraints.push(Constraint::Length(1)); // active mode

    let show_workers = !model.workers.is_empty();
    if show_workers {
        constraints.push(Constraint::Min(0));
    }

    let show_escalations = !model.escalations.is_empty();
    if show_escalations {
        constraints.push(Constraint::Min(0));
    }

    constraints.push(Constraint::Length(1)); // cost
    constraints.push(Constraint::Length(1)); // footer

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;
    sections::header::render(model, frame, chunks[idx], theme);
    idx += 1;

    if show_classifier {
        sections::classifier::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    sections::active_mode::render(model, frame, chunks[idx], theme);
    idx += 1;

    if show_workers {
        sections::workers::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    if show_escalations {
        sections::escalations::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    sections::cost::render(model, frame, chunks[idx], theme);
    idx += 1;
    sections::footer::render(model, frame, chunks[idx], theme);
}

fn render_expanded(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let mut constraints = vec![Constraint::Length(1)]; // header

    let show_classifier = model.classifier.is_some();
    if show_classifier {
        constraints.push(Constraint::Min(0));
    }

    constraints.push(Constraint::Length(1)); // active mode

    let show_plan = model.plan.is_some();
    if show_plan {
        constraints.push(Constraint::Min(0));
    }

    let show_workers = !model.workers.is_empty();
    if show_workers {
        constraints.push(Constraint::Min(0));
    }

    let show_escalations = !model.escalations.is_empty();
    if show_escalations {
        constraints.push(Constraint::Min(0));
    }

    let show_gates = model.active_mode == ActiveMode::GoalRun && !model.evidence_gates.is_empty();
    if show_gates {
        constraints.push(Constraint::Min(0));
    }

    let show_slices = model.active_mode == ActiveMode::GoalRun && !model.slices.is_empty();
    if show_slices {
        constraints.push(Constraint::Min(0));
    }

    constraints.push(Constraint::Length(1)); // cost
    constraints.push(Constraint::Length(1)); // footer

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;
    sections::header::render(model, frame, chunks[idx], theme);
    idx += 1;

    if show_classifier {
        sections::classifier::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    sections::active_mode::render(model, frame, chunks[idx], theme);
    idx += 1;

    if show_plan {
        sections::plan::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    if show_workers {
        sections::workers::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    if show_escalations {
        sections::escalations::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    if show_gates {
        sections::evidence_gates::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    if show_slices {
        sections::slices::render(model, frame, chunks[idx], theme);
        idx += 1;
    }

    sections::cost::render(model, frame, chunks[idx], theme);
    idx += 1;
    sections::footer::render(model, frame, chunks[idx], theme);
}

fn active_mode_word(model: &PaneModel) -> String {
    use crate::vis::bus::ActiveMode;
    match model.active_mode {
        ActiveMode::Idle => "idle".into(),
        ActiveMode::DirectLlm => "direct-llm".into(),
        ActiveMode::WireWorker => "wire-worker".into(),
        ActiveMode::PlannerWorkers => "planner+workers".into(),
        ActiveMode::GoalRun => {
            if model.has_failed_gate() {
                "gate failed".into()
            } else {
                "goal-run".into()
            }
        }
    }
}
