use super::test_goal;
use crate::runtime::db::handle::DbHandle;
use crate::runtime::db::repo::{decision::DecisionRepo, goal::GoalRepo};
use crate::runtime::db::types::DecisionRecord;

fn test_decision(goal_id: &str, kind: &str) -> DecisionRecord {
    DecisionRecord {
        decision_id: None,
        goal_id: goal_id.to_string(),
        version: 1,
        actor: "controller".to_string(),
        phase: "planning".to_string(),
        kind: kind.to_string(),
        decision: "proceed".to_string(),
        rationale: "rationale".to_string(),
        constraints: Some("[\"c1\"]".to_string()),
        artifacts: Some("[\"/tmp/a\"]".to_string()),
        created_at: 1_700_000_000,
    }
}

#[tokio::test]
async fn test_decision_append_and_get() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    for i in 0..5 {
        let d = test_decision("goal-1", &format!("kind-{}", i));
        db.decision_repo().append(&d).await.unwrap();
    }

    let fetched = db.decision_repo().get_by_goal("goal-1").await.unwrap();
    assert_eq!(fetched.len(), 5);
    assert_eq!(fetched[0].kind, "kind-0");
    assert_eq!(fetched[4].kind, "kind-4");

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_decision_get_by_missing_goal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let decisions = db
        .decision_repo()
        .get_by_goal("no-such-goal")
        .await
        .unwrap();
    assert!(decisions.is_empty());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_decision_cascading_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.decision_repo()
        .append(&test_decision("goal-1", "k"))
        .await
        .unwrap();

    db.goal_repo().delete("goal-1").await.unwrap();

    let fetched = db.decision_repo().get_by_goal("goal-1").await.unwrap();
    assert!(fetched.is_empty());

    db.close().await.unwrap();
}
