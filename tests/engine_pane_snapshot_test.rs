#![cfg(feature = "tui")]

use std::path::PathBuf;

use ratatui::{backend::TestBackend, Terminal};

use omk::vis::bus::{ActiveMode, EngineEvent, Intent};
use omk::vis::engine::state::PaneStateMachine;
use omk::vis::engine::{render, PaneModel, PaneState, Theme};

fn render_scenario(
    events: &[EngineEvent],
    state: PaneState,
    width: u16,
    height: u16,
) -> Vec<String> {
    let mut model = PaneModel {
        state,
        ..PaneModel::default()
    };
    for ev in events {
        model.apply(ev.clone());
    }

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            render(&model, f, area, &Theme::Dark);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut lines = Vec::with_capacity(height as usize);
    for y in 0..buffer.area.height {
        let mut line = String::with_capacity(width as usize);
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line);
    }
    lines
}

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from("tests/fixtures/engine_pane").join(format!("{name}.snap"))
}

fn check_snapshot(name: &str, actual: &[String]) {
    let path = snapshot_path(name);
    let actual_text = actual.join("\n") + "\n";

    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok();

    if update || !path.exists() {
        std::fs::write(&path, &actual_text).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        expected, actual_text,
        "snapshot mismatch for {name}\n\nexpected:\n{expected}\n\nactual:\n{actual_text}"
    );
}

fn read_fixture(name: &str) -> Vec<EngineEvent> {
    let path = PathBuf::from("tests/fixtures/engine_pane").join(format!("{name}.jsonl"));
    if !path.exists() {
        return Vec::new();
    }
    let content = std::fs::read_to_string(&path).unwrap();
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

// ---------------------------------------------------------------------------
// Snapshot scenarios
// ---------------------------------------------------------------------------

#[test]
fn scenario_collapsed_idle() {
    let events = read_fixture("scenario_collapsed_idle");
    let lines = render_scenario(&events, PaneState::Collapsed, 80, 24);
    check_snapshot("scenario_collapsed_idle", &lines);
}

#[test]
fn scenario_compact_classifier_decided() {
    let events = read_fixture("scenario_compact_classifier_decided");
    let lines = render_scenario(&events, PaneState::Compact, 80, 24);
    check_snapshot("scenario_compact_classifier_decided", &lines);
}

#[test]
fn scenario_expanded_small_worker_running() {
    let events = read_fixture("scenario_expanded_small_worker_running");
    let lines = render_scenario(&events, PaneState::Expanded, 80, 24);
    check_snapshot("scenario_expanded_small_worker_running", &lines);
}

#[test]
fn scenario_expanded_medium_plan_3of4_done() {
    let events = read_fixture("scenario_expanded_medium_plan_3of4_done");
    let lines = render_scenario(&events, PaneState::Expanded, 80, 24);
    check_snapshot("scenario_expanded_medium_plan_3of4_done", &lines);
}

#[test]
fn scenario_expanded_large_goal_running() {
    let events = read_fixture("scenario_expanded_large_goal_running");
    let lines = render_scenario(&events, PaneState::Expanded, 80, 24);
    check_snapshot("scenario_expanded_large_goal_running", &lines);
}

#[test]
fn scenario_cost_meter_accumulates_correctly() {
    let events = read_fixture("scenario_cost_meter_accumulates_correctly");
    let lines = render_scenario(&events, PaneState::Expanded, 80, 24);
    check_snapshot("scenario_cost_meter_accumulates_correctly", &lines);
}

#[test]
fn scenario_empty_sections_hide_correctly() {
    let events = read_fixture("scenario_empty_sections_hide_correctly");
    let lines = render_scenario(&events, PaneState::Expanded, 80, 24);
    check_snapshot("scenario_empty_sections_hide_correctly", &lines);
}

// ---------------------------------------------------------------------------
// State-machine scenarios
// ---------------------------------------------------------------------------

#[test]
fn scenario_state_machine_auto_expands_on_router_escalating() {
    let mut sm = PaneStateMachine::new();
    sm.state = PaneState::Collapsed;
    let now = std::time::Instant::now();
    sm.on_event(
        &EngineEvent::RouterEscalating {
            intent: Intent::Small,
            target_mode: ActiveMode::WireWorker,
            preflight: false,
        },
        now,
    );
    assert_eq!(sm.state(), PaneState::Compact);
}

#[test]
fn scenario_state_machine_auto_collapses_after_idle() {
    let mut sm = PaneStateMachine::new();
    sm.state = PaneState::Expanded;
    let now = std::time::Instant::now();
    sm.last_event_at = now - std::time::Duration::from_secs(70);
    sm.on_tick(now);
    assert_eq!(sm.state(), PaneState::Collapsed);
}

// ---------------------------------------------------------------------------
// Unit helpers exercised through the snapshot pipeline
// ---------------------------------------------------------------------------

#[test]
fn model_apply_classifier_truncates_recent() {
    let mut model = PaneModel::default();
    for i in 0..7 {
        model.apply(EngineEvent::ClassifierDecided {
            intent: Intent::Trivial,
            confidence: 0.5,
            latency_ms: i,
            reasoning: format!("r{i}"),
        });
    }
    assert_eq!(model.recent_classifications.len(), 5);
}

#[test]
fn model_cost_accumulation() {
    let mut model = PaneModel::default();
    model.apply(EngineEvent::CostDelta {
        source: "a".into(),
        tokens_in: 100,
        tokens_out: 50,
        usd: 0.012,
    });
    model.apply(EngineEvent::CostDelta {
        source: "b".into(),
        tokens_in: 200,
        tokens_out: 100,
        usd: 0.030,
    });
    assert_eq!(model.cost.tokens_in, 300);
    assert_eq!(model.cost.tokens_out, 150);
    assert!((model.cost.usd - 0.042).abs() < 0.0001);
}
