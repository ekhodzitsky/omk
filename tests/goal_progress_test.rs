use std::path::PathBuf;

use chrono::Utc;
use omk::runtime::goal::{
    GoalArtifact, GoalPhase, GoalProgressLineKind, GoalProgressSnapshot, GoalState, GoalStatus,
    GoalTerminalCriteria,
};
use omk::vis::goal_progress::render_goal_progress;

#[test]
fn progress_snapshot_records_structured_goal_narrative() {
    let snapshot = GoalProgressSnapshot::new(GoalPhase::Execution)
        .current_task("goal-agent-execute")
        .implemented("bounded executor slice")
        .running_verification("cargo test --test goal_progress_test")
        .review_blocker("secret scan")
        .next_step("integrate slice N")
        .blocked("missing credential")
        .ready("baseline commit abc123")
        .proof_path(PathBuf::from(".omk/goals/goal-1/proof.json"));

    assert_eq!(snapshot.phase, GoalPhase::Execution);
    assert_eq!(snapshot.current_task.as_deref(), Some("goal-agent-execute"));
    assert_eq!(snapshot.done, vec!["bounded executor slice"]);
    assert_eq!(snapshot.next, vec!["integrate slice N"]);
    assert_eq!(snapshot.blockers, vec!["secret scan", "missing credential"]);
    assert_eq!(snapshot.gates, vec!["cargo test --test goal_progress_test"]);
    assert_eq!(snapshot.reviews, vec!["secret scan"]);
    assert_eq!(
        snapshot.narrative[0].kind,
        GoalProgressLineKind::Implemented
    );
    assert_eq!(
        snapshot.narrative[0].render(),
        "implemented bounded executor slice"
    );
    assert!(snapshot
        .narrative
        .iter()
        .any(|line| line.render() == "running verification cargo test --test goal_progress_test"));
    assert!(snapshot
        .narrative
        .iter()
        .any(|line| line.render() == "review found blocker secret scan, creating fix task"));
    assert!(snapshot
        .narrative
        .iter()
        .any(|line| line.render() == "next: integrate slice N"));
    assert!(snapshot
        .narrative
        .iter()
        .any(|line| line.render() == "ready: baseline commit abc123"));
}

#[test]
fn terminal_progress_render_is_structured_not_chat() {
    let snapshot = GoalProgressSnapshot::new(GoalPhase::Proof)
        .current_task("goal-proof")
        .implemented("review wall evidence")
        .running_verification("cargo check --all-targets")
        .next_step("publish proof bundle")
        .ready("baseline commit abc123")
        .proof_path(PathBuf::from(".omk/goals/goal-1/proof.json"));

    let text = render_goal_progress(&snapshot);

    assert!(text.contains("OMK goal progress"));
    assert!(text.contains("phase: proof"));
    assert!(text.contains("current task: goal-proof"));
    assert!(text.contains("done:"));
    assert!(text.contains("- implemented review wall evidence"));
    assert!(text.contains("gates:"));
    assert!(text.contains("- running verification cargo check --all-targets"));
    assert!(text.contains("next:"));
    assert!(text.contains("- publish proof bundle"));
    assert!(text.contains("proof: .omk/goals/goal-1/proof.json"));
    assert!(text.contains("narrative:"));
    assert!(text.contains("- ready: baseline commit abc123"));
    assert!(!text.contains("assistant:"));
    assert!(!text.contains("user:"));
}

#[test]
fn progress_snapshot_from_goal_state_keeps_phase_status_and_proof_path() {
    let state_dir = PathBuf::from(".omk/goals/goal-20260514-test");
    let state = GoalState {
        version: 1,
        goal_id: "goal-20260514-test".to_string(),
        original_goal: "Make omk goal narrative visible".to_string(),
        normalized_goal: "Make omk goal narrative visible".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Planning,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        until_ready: true,
        budget_time: None,
        budget_tokens: None,
        budget_usd: None,
        max_agents: Some(6),
        terminal_criteria: GoalTerminalCriteria::default(),
        artifacts: vec![GoalArtifact {
            kind: "proof".to_string(),
            path: PathBuf::from("proof.json"),
            created_at: Utc::now(),
        }],
        failure: None,
        state_dir: state_dir.clone(),
        cost_tracker_path: None,
    };

    let snapshot = GoalProgressSnapshot::from_goal_state(&state);

    assert_eq!(snapshot.phase, GoalPhase::Planning);
    assert_eq!(snapshot.current_task.as_deref(), Some("goal-planning"));
    assert_eq!(snapshot.proof_path, Some(state_dir.join("proof.json")));
    assert!(snapshot
        .narrative
        .iter()
        .any(|line| line.render() == "next: write PRD, technical plan, and test spec"));
}
