use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::vis::bus::ActiveMode;
use crate::vis::engine::model::PaneModel;
use crate::vis::engine::theme::Theme;

pub fn render(model: &PaneModel, frame: &mut Frame, area: Rect, theme: Theme) {
    let label = active_mode_label(model);
    let line = Line::from(vec![
        Span::styled("mode: ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            label,
            Style::default()
                .fg(theme.fg_emphasis())
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn active_mode_label(model: &PaneModel) -> String {
    match model.active_mode {
        ActiveMode::Idle => "idle".into(),
        ActiveMode::DirectLlm => "direct-llm".into(),
        ActiveMode::WireWorker => "wire-worker".into(),
        ActiveMode::PlannerWorkers => "planner+workers".into(),
        ActiveMode::GoalRun => {
            let phase = goal_phase(model);
            if let Some(ref gid) = model.goal_id {
                format!("goal-run · {} · {}", gid, phase)
            } else {
                "goal-run".into()
            }
        }
    }
}

fn goal_phase(model: &PaneModel) -> &'static str {
    if model.evidence_gates.values().any(|g| g.state == "failed") {
        return "blocked";
    }
    if model
        .evidence_gates
        .values()
        .any(|g| g.state == "passed" && g.gate == "proof")
    {
        return "reviewing";
    }
    if model
        .workers
        .values()
        .any(|w| w.status == crate::vis::engine::blocks::WorkerStatus::Running)
    {
        return "executing";
    }
    if model
        .evidence_gates
        .values()
        .any(|g| g.state == "running" || g.state == "gating")
    {
        return "verifying";
    }
    if model.evidence_gates.values().all(|g| g.state == "passed")
        && !model.evidence_gates.is_empty()
    {
        return "ready";
    }
    if model.plan.is_some() {
        return "planning";
    }
    "planning"
}
