use super::handle::DbHandle;
use super::types::{GoalRecord, TaskRecord};

pub(crate) fn test_goal() -> GoalRecord {
    GoalRecord {
        goal_id: "goal-1".to_string(),
        status: "active".to_string(),
        phase: "planning".to_string(),
        kind: Some("feature".to_string()),
        original_goal: "Implement sqlite module".to_string(),
        normalized_goal: "implement sqlite module".to_string(),
        goal_text: "Implement sqlite module".to_string(),
        project_dir: "/tmp/test".to_string(),
        state_dir: "/tmp/test/.omk/state".to_string(),
        policy: "local".to_string(),
        delivery_policy: "manual".to_string(),
        merge_policy: "disabled".to_string(),
        until_ready: false,
        slice_execution: false,
        max_agents: Some(4),
        budget_time_secs: Some(3600),
        budget_tokens: Some(1_000_000),
        budget_usd: Some(500),
        cost_tracker_path: None,
        terminal_criteria: None,
        failure: None,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        completed_at: None,
        controller_pid: Some(1234),
        version: 1,
    }
}

pub(crate) fn test_task(id: &str, goal_id: &str) -> TaskRecord {
    TaskRecord {
        task_id: id.to_string(),
        goal_id: goal_id.to_string(),
        title: id.to_string(),
        description: "A test task".to_string(),
        kind: "code".to_string(),
        status: "pending".to_string(),
        owner: Some("agent-1".to_string()),
        read_set: None,
        write_set: Some("[\"src/lib.rs\"]".to_string()),
        depends_on: None,
        risk: "low".to_string(),
        acceptance: None,
        evidence: None,
        retry_count: 0,
        max_retries: 3,
        lease_expires_at: None,
        completed_at: None,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
    }
}

#[tokio::test]
async fn test_open_create_migrate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    assert!(path.exists());

    let journal = db
        .conn
        .call(|conn| Ok(conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?))
        .await
        .unwrap();
    assert_eq!(journal.to_lowercase(), "wal");

    let fk = db
        .conn
        .call(|conn| Ok(conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i32>(0))?))
        .await
        .unwrap();
    assert_eq!(fk, 1);

    db.close().await.unwrap();
}

mod concurrent;
mod crud;
mod misc;
mod transaction;
