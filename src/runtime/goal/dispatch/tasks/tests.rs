use super::run_goal_agent_task_wave;
use crate::runtime::goal::agent::{
    GoalAgentDispatchPlan, GoalAgentTaskProposal, GoalAgentWaveKind,
};
use crate::runtime::goal::proof::write_json_artifact;
use crate::runtime::goal::state::{
    GoalPhase, GoalState, GoalStatus, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_TASK_GRAPH_FILE,
};
use crate::runtime::goal::task_graph::{
    GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus,
};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Once;
use tempfile::tempdir;
use tokio::fs;

static INIT: Once = Once::new();

fn ensure_short_lease() {
    INIT.call_once(|| {
        std::env::set_var("OMK_GOAL_AGENT_LEASE_SECS", "1");
    });
}

fn test_proposal(id: &str, budget_secs: u64, write_set: &[&str]) -> GoalAgentTaskProposal {
    GoalAgentTaskProposal {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Description {id}"),
        dependencies: vec![],
        read_set: vec![],
        write_set: write_set.iter().map(|s| s.to_string()).collect(),
        risk: "low".to_string(),
        acceptance: vec!["accept".to_string()],
        budget_secs,
        priority: 0,
    }
}

fn done_task(id: &str) -> GoalTask {
    GoalTask {
        id: id.to_string(),
        title: id.to_string(),
        description: id.to_string(),
        status: GoalTaskStatus::Done,
        owner_role: None,
        completed_at: Some(Utc::now()),
        evidence: vec![GoalTaskEvidence {
            kind: "artifact".to_string(),
            path: PathBuf::from("test.md"),
            summary: "test".to_string(),
        }],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec![],
        read_set: vec![],
        write_set: vec![],
        risk: "low".to_string(),
        acceptance: vec!["accept".to_string()],
    }
}

async fn setup_goal_state(budget_time: Option<String>) -> (GoalState, GoalTaskGraph, PathBuf) {
    let tmp = tempdir().unwrap();
    let state_dir = tmp.path().join("goal-state");
    fs::create_dir_all(&state_dir).await.unwrap();
    let project_dir = tmp.path().join("project");
    fs::create_dir_all(&project_dir).await.unwrap();

    let state = GoalState {
        version: 1,
        goal_id: "goal-test".to_string(),
        original_goal: "test goal".to_string(),
        normalized_goal: "test goal".to_string(),
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
        state_dir: state_dir.clone(),
    };
    state.save().await.unwrap();

    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "goal-test".to_string(),
        generated_at: Utc::now(),
        tasks: vec![done_task(GOAL_AGENT_EXECUTE_TASK_ID)],
    };
    write_json_artifact(&state_dir.join(GOAL_TASK_GRAPH_FILE), &task_graph)
        .await
        .unwrap();

    (state, task_graph, project_dir)
}

#[tokio::test]
async fn task_rejected_when_budget_exceeded() {
    ensure_short_lease();
    let (state, task_graph, project_dir) = setup_goal_state(Some("10s".to_string())).await;
    let proposal = test_proposal("task-a", 120, &["README.md"]);
    let dispatch = GoalAgentDispatchPlan {
        run_key: "run-1".to_string(),
        kind: GoalAgentWaveKind::Initial,
        proposals: vec![proposal],
        allow_existing_task_ids: false,
    };

    let evidence =
        run_goal_agent_task_wave(&state, &task_graph, &project_dir, Utc::now(), &dispatch)
            .await
            .unwrap();
    assert_eq!(evidence.accepted_task_count, 0);
    assert_eq!(evidence.rejected_task_count, 1);
    assert!(evidence
        .worker_summary
        .as_ref()
        .unwrap()
        .contains("rejected all proposed agent tasks"));

    let events_path = state.state_dir.join(crate::runtime::config::EVENTS_FILE);
    let events_content = fs::read_to_string(&events_path).await.unwrap();
    let events: Vec<serde_json::Value> = events_content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let proposed = events
        .iter()
        .find(|e| e.get("kind").and_then(|k| k.as_str()) == Some("task_proposed"))
        .expect("task_proposed event");
    assert_eq!(proposed["payload"]["task_id"], "task-a");

    let rejected = events
        .iter()
        .find(|e| e.get("kind").and_then(|k| k.as_str()) == Some("task_rejected"))
        .expect("task_rejected event");
    assert_eq!(rejected["payload"]["task_id"], "task-a");
    let reason = rejected["payload"]["reason"].as_str().unwrap();
    assert!(
        reason.contains("would exceed goal time budget"),
        "reason: {}",
        reason
    );
}

#[tokio::test]
async fn task_rejected_when_path_policy_violated() {
    ensure_short_lease();
    let (state, task_graph, project_dir) = setup_goal_state(None).await;
    let proposal = test_proposal("task-b", 120, &["/absolute/path.md"]);
    let dispatch = GoalAgentDispatchPlan {
        run_key: "run-2".to_string(),
        kind: GoalAgentWaveKind::Initial,
        proposals: vec![proposal],
        allow_existing_task_ids: false,
    };

    let evidence =
        run_goal_agent_task_wave(&state, &task_graph, &project_dir, Utc::now(), &dispatch)
            .await
            .unwrap();
    assert_eq!(evidence.accepted_task_count, 0);
    assert_eq!(evidence.rejected_task_count, 1);

    let events_path = state.state_dir.join(crate::runtime::config::EVENTS_FILE);
    let events_content = fs::read_to_string(&events_path).await.unwrap();
    let events: Vec<serde_json::Value> = events_content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let rejected = events
        .iter()
        .find(|e| e.get("kind").and_then(|k| k.as_str()) == Some("task_rejected"))
        .expect("task_rejected event");
    let reason = rejected["payload"]["reason"].as_str().unwrap();
    assert!(
        reason.contains("outside the allowed goal policy roots"),
        "reason: {}",
        reason
    );
}

#[tokio::test]
async fn accepted_task_writes_deterministic_accepted_event() {
    ensure_short_lease();
    let (state, task_graph, project_dir) = setup_goal_state(Some("1h".to_string())).await;
    let proposal = test_proposal("task-c", 120, &["README.md"]);
    let dispatch = GoalAgentDispatchPlan {
        run_key: "run-3".to_string(),
        kind: GoalAgentWaveKind::Initial,
        proposals: vec![proposal],
        allow_existing_task_ids: false,
    };

    let evidence =
        run_goal_agent_task_wave(&state, &task_graph, &project_dir, Utc::now(), &dispatch)
            .await
            .unwrap();
    assert_eq!(evidence.accepted_task_count, 1);
    assert_eq!(evidence.rejected_task_count, 0);

    let events_path = state.state_dir.join(crate::runtime::config::EVENTS_FILE);
    let events_content = fs::read_to_string(&events_path).await.unwrap();
    let events: Vec<serde_json::Value> = events_content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let accepted = events
        .iter()
        .find(|e| e.get("kind").and_then(|k| k.as_str()) == Some("task_accepted"))
        .expect("task_accepted event");
    assert_eq!(accepted["payload"]["task_id"], "task-c");
    assert!(accepted["payload"]["budget_snapshot"].is_object());
    assert_eq!(
        accepted["payload"]["budget_snapshot"]["task_budget_secs"],
        120
    );
}

#[tokio::test]
async fn rejected_task_does_not_change_execution_state_as_completed() {
    ensure_short_lease();
    let (state, task_graph, project_dir) = setup_goal_state(Some("10s".to_string())).await;
    let proposal = test_proposal("task-d", 120, &["README.md"]);
    let dispatch = GoalAgentDispatchPlan {
        run_key: "run-4".to_string(),
        kind: GoalAgentWaveKind::Initial,
        proposals: vec![proposal],
        allow_existing_task_ids: false,
    };

    let evidence =
        run_goal_agent_task_wave(&state, &task_graph, &project_dir, Utc::now(), &dispatch)
            .await
            .unwrap();
    assert_eq!(evidence.summary.completed, 0);
    assert_eq!(evidence.summary.failed, 1);
    assert_eq!(evidence.summary.total, 1);
    assert_eq!(evidence.accepted_task_count, 0);
}
