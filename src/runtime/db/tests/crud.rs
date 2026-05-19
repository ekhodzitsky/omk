use super::{test_goal, test_task};
use crate::runtime::db::handle::DbHandle;
use crate::runtime::db::repo::{
    artifact::ArtifactRepo, budget::BudgetRepo, event::EventRepo, goal::GoalRepo, proof::ProofRepo,
    task::TaskRepo,
};
use crate::runtime::db::types::{BudgetCheckpoint, GoalFilter, ProofRecord, TaskRecord};

#[tokio::test]
async fn test_goal_crud() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let goal = test_goal();
    db.goal_repo().create(&goal).await.unwrap();

    let fetched = db.goal_repo().get("goal-1").await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.goal_id, "goal-1");
    assert_eq!(fetched.status, "active");

    db.goal_repo()
        .update_status("goal-1", "completed", "done")
        .await
        .unwrap();
    let updated = db.goal_repo().get("goal-1").await.unwrap().unwrap();
    assert_eq!(updated.status, "completed");
    assert_eq!(updated.phase, "done");

    let list = db
        .goal_repo()
        .list(GoalFilter {
            status: Some("completed".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].goal_id, "goal-1");

    db.goal_repo().delete("goal-1").await.unwrap();
    let gone = db.goal_repo().get("goal-1").await.unwrap();
    assert!(gone.is_none());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_goal_update_and_delete_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let err = db
        .goal_repo()
        .update_status("missing", "x", "y")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("goal not found"));

    let err = db
        .goal_repo()
        .delete("missing")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("goal not found"));

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_task_batch_create() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    let tasks: Vec<TaskRecord> = (0..10)
        .map(|i| test_task(&format!("task-{}", i), "goal-1"))
        .collect();
    db.task_repo().create_batch("goal-1", &tasks).await.unwrap();

    let fetched = db.task_repo().get_by_goal("goal-1").await.unwrap();
    assert_eq!(fetched.len(), 10);

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_task_batch_create_mismatched_goal_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    let tasks = vec![test_task("t1", "wrong-goal")];
    let err = db
        .task_repo()
        .create_batch("goal-1", &tasks)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("not all tasks belong to goal"));

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_task_graph_update() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    let initial = vec![test_task("task-a", "goal-1"), test_task("task-b", "goal-1")];
    db.task_repo()
        .create_batch("goal-1", &initial)
        .await
        .unwrap();

    let replacement = vec![test_task("task-c", "goal-1"), test_task("task-d", "goal-1")];
    db.task_repo()
        .update_task_graph("goal-1", &replacement)
        .await
        .unwrap();

    let fetched = db.task_repo().get_by_goal("goal-1").await.unwrap();
    assert_eq!(fetched.len(), 2);
    let ids: Vec<String> = fetched.into_iter().map(|t| t.task_id).collect();
    assert!(ids.contains(&"task-c".to_string()));
    assert!(ids.contains(&"task-d".to_string()));

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_task_update_status() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.task_repo()
        .create_batch("goal-1", &[test_task("t1", "goal-1")])
        .await
        .unwrap();

    db.task_repo().update_status("t1", "running").await.unwrap();
    let tasks = db.task_repo().get_by_goal("goal-1").await.unwrap();
    assert_eq!(tasks[0].status, "running");

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_task_get_by_id_and_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.task_repo()
        .create_batch("goal-1", &[test_task("t1", "goal-1")])
        .await
        .unwrap();

    let found = db.task_repo().get_by_id("t1").await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().task_id, "t1");

    let missing = db.task_repo().get_by_id("no-such-task").await.unwrap();
    assert!(missing.is_none());

    let err = db
        .task_repo()
        .update_status("no-such-task", "done")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("task not found"));

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_event_append_and_get() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    for i in 0..100 {
        db.event_repo()
            .append("goal-1", "test", &format!("payload-{}", i))
            .await
            .unwrap();
    }

    let events = db
        .event_repo()
        .get_by_goal("goal-1", None, None)
        .await
        .unwrap();
    assert_eq!(events.len(), 100);
    for (i, ev) in events.iter().enumerate() {
        assert_eq!(ev.payload, format!("payload-{}", i));
    }

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_event_pagination() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    for i in 0..20 {
        db.event_repo()
            .append("goal-1", "test", &format!("payload-{}", i))
            .await
            .unwrap();
    }

    let page = db
        .event_repo()
        .get_by_goal("goal-1", None, Some(5))
        .await
        .unwrap();
    assert_eq!(page.len(), 5);

    let since = page.last().unwrap().created_at;
    let rest = db
        .event_repo()
        .get_by_goal("goal-1", Some(since), None)
        .await
        .unwrap();
    // Since timestamps may collide at second granularity, >= includes the boundary.
    assert!(rest.len() >= 15);

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_event_delete_by_goal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.event_repo().append("goal-1", "a", "1").await.unwrap();
    db.event_repo().append("goal-1", "b", "2").await.unwrap();

    db.event_repo().delete_by_goal("goal-1").await.unwrap();
    let events = db
        .event_repo()
        .get_by_goal("goal-1", None, None)
        .await
        .unwrap();
    assert!(events.is_empty());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_event_get_by_missing_goal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let events = db
        .event_repo()
        .get_by_goal("no-such-goal", None, None)
        .await
        .unwrap();
    assert!(events.is_empty());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_proof_upsert() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    let proof = ProofRecord {
        goal_id: "goal-1".to_string(),
        version: 1,
        status: "pending".to_string(),
        readiness: "not_ready".to_string(),
        summary: "Proof pending".to_string(),
        task_graph_summary: None,
        changed_files: Some("[\"src/lib.rs\"]".to_string()),
        commits: None,
        git: None,
        gates: None,
        gates_passed: 0,
        gates_total: 3,
        post_mutation_gates_ran: false,
        known_gaps: None,
        human_decisions_required: None,
        recovery_status: None,
        generated_at: 1_700_000_000,
    };
    db.proof_repo().upsert(&proof).await.unwrap();

    let fetched = db.proof_repo().get("goal-1").await.unwrap().unwrap();
    assert_eq!(fetched.status, "pending");
    assert_eq!(fetched.gates_total, 3);

    let updated = ProofRecord {
        goal_id: "goal-1".to_string(),
        version: 1,
        status: "passed".to_string(),
        readiness: "ready".to_string(),
        summary: "All gates passed".to_string(),
        task_graph_summary: None,
        changed_files: Some("[\"src/lib.rs\", \"src/main.rs\"]".to_string()),
        commits: None,
        git: None,
        gates: None,
        gates_passed: 3,
        gates_total: 3,
        post_mutation_gates_ran: false,
        known_gaps: None,
        human_decisions_required: None,
        recovery_status: Some("none".to_string()),
        generated_at: 1_700_000_001,
    };
    db.proof_repo().upsert(&updated).await.unwrap();

    let fetched = db.proof_repo().get("goal-1").await.unwrap().unwrap();
    assert_eq!(fetched.status, "passed");
    assert_eq!(fetched.gates_passed, 3);

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_proof_get_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let missing = db.proof_repo().get("no-such-goal").await.unwrap();
    assert!(missing.is_none());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_budget_checkpoints() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    for i in 0..5 {
        let cp = BudgetCheckpoint {
            checkpoint_id: None,
            goal_id: "goal-1".to_string(),
            version: 1,
            label: "tokens".to_string(),
            status: "active".to_string(),
            phase: "planning".to_string(),
            recorded_at: 1_700_000_000 + i as i64,
            budget_time: None,
            total_budget_secs: Some(3600),
            elapsed_since_created_secs: 0,
            remaining_budget_secs: Some(3600),
            budget_tokens: Some(1_000_000),
            used_tokens: (i * 100) as i64,
            remaining_budget_tokens: Some(1_000_000 - (i * 100) as i64),
            budget_usd: Some(500),
            estimated_cost_usd: 0,
            remaining_budget_usd: Some(500),
            limit_value: Some(1_000_000),
            used_value: Some((i * 100) as i64),
            created_at: 1_700_000_000 + i as i64,
        };
        db.budget_repo().append_checkpoint(&cp).await.unwrap();
    }

    let fetched = db.budget_repo().get_by_goal("goal-1").await.unwrap();
    assert_eq!(fetched.len(), 5);
    for (i, cp) in fetched.iter().enumerate() {
        assert_eq!(cp.used_tokens, (i * 100) as i64);
    }

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_budget_get_by_missing_goal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let checkpoints = db.budget_repo().get_by_goal("no-such-goal").await.unwrap();
    assert!(checkpoints.is_empty());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_artifact_register() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    db.artifact_repo()
        .register("goal-1", "log", "/tmp/out.log", Some("text/plain"))
        .await
        .unwrap();
    db.artifact_repo()
        .register("goal-1", "screenshot", "/tmp/out.png", Some("image/png"))
        .await
        .unwrap();
    db.artifact_repo()
        .register("goal-1", "log", "/tmp/out2.log", Some("text/plain"))
        .await
        .unwrap();

    let all = db
        .artifact_repo()
        .get_by_goal("goal-1", None)
        .await
        .unwrap();
    assert_eq!(all.len(), 3);

    let logs = db
        .artifact_repo()
        .get_by_goal("goal-1", Some("log"))
        .await
        .unwrap();
    assert_eq!(logs.len(), 2);

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_artifact_get_by_missing_goal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let artifacts = db
        .artifact_repo()
        .get_by_goal("no-such-goal", None)
        .await
        .unwrap();
    assert!(artifacts.is_empty());

    db.close().await.unwrap();
}
