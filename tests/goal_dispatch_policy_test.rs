use chrono::Utc;
use omk::runtime::goal::{
    evaluate_task_budget, GoalAgentTaskProposal, GoalPhase, GoalState, GoalStatus,
    PerTaskBudgetSnapshot,
};
use std::path::PathBuf;

fn test_proposal(id: &str, budget_secs: u64) -> GoalAgentTaskProposal {
    GoalAgentTaskProposal {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Description {id}"),
        dependencies: vec![],
        read_set: vec![],
        write_set: vec!["README.md".to_string()],
        risk: "low".to_string(),
        acceptance: vec!["accept".to_string()],
        budget_secs,
        priority: 0,
    }
}

fn test_state(budget_time: Option<String>) -> GoalState {
    GoalState {
        version: 1,
        goal_id: "goal-test".to_string(),
        original_goal: "test".to_string(),
        normalized_goal: "test".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        until_ready: false,
        budget_time,
        budget_tokens: None,
        budget_usd: None,
        max_agents: Some(1),
        terminal_criteria: Default::default(),
        artifacts: vec![],
        failure: None,
        state_dir: PathBuf::from("/tmp/goal-test-does-not-exist"),
        cost_tracker_path: None,
    }
}

#[tokio::test]
async fn task_rejected_when_budget_exceeded() {
    let state = test_state(Some("10s".to_string()));
    let proposal = test_proposal("task-a", 120);

    let result = evaluate_task_budget(&state, &proposal).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("would exceed goal time budget"),
        "unexpected error: {}",
        err
    );
}

#[tokio::test]
async fn task_accepted_when_budget_available() {
    let state = test_state(Some("1h".to_string()));
    let proposal = test_proposal("task-b", 120);

    let result = evaluate_task_budget(&state, &proposal).await;
    assert!(result.is_ok());
    let snapshot: PerTaskBudgetSnapshot = result.unwrap();
    assert_eq!(snapshot.task_budget_secs, 120);
    assert!(snapshot.remaining_budget_secs.unwrap_or(0) > 0);
}

#[tokio::test]
async fn task_accepted_when_budget_unbounded() {
    let state = test_state(None);
    let proposal = test_proposal("task-c", 120);

    let result = evaluate_task_budget(&state, &proposal).await;
    assert!(result.is_ok());
    let snapshot: PerTaskBudgetSnapshot = result.unwrap();
    assert_eq!(snapshot.total_budget_secs, None);
    assert_eq!(snapshot.remaining_budget_secs, None);
}
