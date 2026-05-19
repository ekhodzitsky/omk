use std::time::Duration;

use super::handle::DbHandle;
use super::repo::{
    artifact::ArtifactRepo, budget::BudgetRepo, event::EventRepo, goal::GoalRepo, proof::ProofRepo,
    task::TaskRepo,
};
use super::types::{BudgetCheckpoint, GoalFilter, GoalRecord, ProofRecord, TaskRecord};

fn test_goal() -> GoalRecord {
    GoalRecord {
        goal_id: "goal-1".to_string(),
        status: "active".to_string(),
        phase: "planning".to_string(),
        kind: Some("feature".to_string()),
        goal_text: "Implement sqlite module".to_string(),
        project_dir: "/tmp/test".to_string(),
        policy: "local".to_string(),
        merge_policy: "disabled".to_string(),
        slice_execution: false,
        max_agents: Some(4),
        budget_time_secs: Some(3600),
        budget_tokens: Some(1_000_000),
        budget_usd: Some(5.0),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        controller_pid: Some(1234),
        version: 1,
    }
}

fn test_task(id: &str, goal_id: &str) -> TaskRecord {
    TaskRecord {
        task_id: id.to_string(),
        goal_id: goal_id.to_string(),
        kind: "code".to_string(),
        status: "pending".to_string(),
        owner: Some("agent-1".to_string()),
        write_set: Some("[\"src/lib.rs\"]".to_string()),
        depends_on: None,
        retry_count: 0,
        max_retries: 3,
        lease_expires_at: None,
        evidence_paths: None,
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
async fn test_proof_upsert() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    let proof = ProofRecord {
        goal_id: "goal-1".to_string(),
        status: "pending".to_string(),
        gates_passed: 0,
        gates_total: 3,
        changed_files: Some("[\"src/lib.rs\"]".to_string()),
        known_gaps: None,
        recovery_status: None,
        generated_at: 1_700_000_000,
    };
    db.proof_repo().upsert(&proof).await.unwrap();

    let fetched = db.proof_repo().get("goal-1").await.unwrap().unwrap();
    assert_eq!(fetched.status, "pending");
    assert_eq!(fetched.gates_total, 3);

    let updated = ProofRecord {
        goal_id: "goal-1".to_string(),
        status: "passed".to_string(),
        gates_passed: 3,
        gates_total: 3,
        changed_files: Some("[\"src/lib.rs\", \"src/main.rs\"]".to_string()),
        known_gaps: None,
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
async fn test_budget_checkpoints() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();

    for i in 0..5 {
        let cp = BudgetCheckpoint {
            checkpoint_id: None,
            goal_id: "goal-1".to_string(),
            kind: "tokens".to_string(),
            limit_value: Some(1_000_000.0),
            used_value: Some((i * 100) as f64),
            created_at: 1_700_000_000 + i as i64,
        };
        db.budget_repo().append_checkpoint(&cp).await.unwrap();
    }

    let fetched = db.budget_repo().get_by_goal("goal-1").await.unwrap();
    assert_eq!(fetched.len(), 5);
    for (i, cp) in fetched.iter().enumerate() {
        assert_eq!(cp.used_value, Some((i * 100) as f64));
    }

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
async fn test_transaction_commit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let tx = db.transaction().await.unwrap();
    tx.goal_repo().create(&test_goal()).await.unwrap();

    // Data visible inside transaction.
    let inside = tx.goal_repo().get("goal-1").await.unwrap();
    assert!(inside.is_some());

    tx.commit().await.unwrap();

    // Data durable after commit.
    let outside = db.goal_repo().get("goal-1").await.unwrap();
    assert!(outside.is_some());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_transaction_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    let tx = db.transaction().await.unwrap();
    tx.goal_repo().create(&test_goal()).await.unwrap();
    tx.rollback().await.unwrap();

    let outside = db.goal_repo().get("goal-1").await.unwrap();
    assert!(outside.is_none());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_cascading_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.task_repo()
        .create_batch("goal-1", &[test_task("t1", "goal-1")])
        .await
        .unwrap();
    db.event_repo()
        .append("goal-1", "start", "{}")
        .await
        .unwrap();
    db.proof_repo()
        .upsert(&ProofRecord {
            goal_id: "goal-1".to_string(),
            status: "pending".to_string(),
            gates_passed: 0,
            gates_total: 0,
            changed_files: None,
            known_gaps: None,
            recovery_status: None,
            generated_at: 1,
        })
        .await
        .unwrap();
    db.budget_repo()
        .append_checkpoint(&BudgetCheckpoint {
            checkpoint_id: None,
            goal_id: "goal-1".to_string(),
            kind: "usd".to_string(),
            limit_value: Some(10.0),
            used_value: Some(1.0),
            created_at: 1,
        })
        .await
        .unwrap();
    db.artifact_repo()
        .register("goal-1", "log", "/tmp/x", None)
        .await
        .unwrap();

    db.goal_repo().delete("goal-1").await.unwrap();

    assert!(db
        .task_repo()
        .get_by_goal("goal-1")
        .await
        .unwrap()
        .is_empty());
    assert!(db
        .event_repo()
        .get_by_goal("goal-1", None, None)
        .await
        .unwrap()
        .is_empty());
    assert!(db.proof_repo().get("goal-1").await.unwrap().is_none());
    assert!(db
        .budget_repo()
        .get_by_goal("goal-1")
        .await
        .unwrap()
        .is_empty());
    assert!(db
        .artifact_repo()
        .get_by_goal("goal-1", None)
        .await
        .unwrap()
        .is_empty());

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();
    db.goal_repo().create(&test_goal()).await.unwrap();

    let mut handles = vec![];
    for i in 0..10 {
        let db = db.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..10 {
                db.event_repo()
                    .append("goal-1", "test", &format!("event-{}-{}", i, j))
                    .await
                    .unwrap();
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let events = db
        .event_repo()
        .get_by_goal("goal-1", None, None)
        .await
        .unwrap();
    assert_eq!(events.len(), 100);

    db.close().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_reads_during_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    // Two independent connections to the same database file.
    let db_writer = DbHandle::open(&path).await.unwrap();
    let db_reader = DbHandle::open(&path).await.unwrap();

    db_writer.goal_repo().create(&test_goal()).await.unwrap();

    for i in 0..50 {
        db_writer
            .event_repo()
            .append("goal-1", "test", &format!("event-{}", i))
            .await
            .unwrap();
    }

    let write_handle = {
        let db = db_writer.clone();
        tokio::spawn(async move {
            for i in 50..100 {
                db.event_repo()
                    .append("goal-1", "test", &format!("event-{}", i))
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
    };

    let read_handle = {
        let db = db_reader.clone();
        tokio::spawn(async move {
            let mut reads = 0;
            for _ in 0..20 {
                let ev = db
                    .event_repo()
                    .get_by_goal("goal-1", None, None)
                    .await
                    .unwrap();
                reads += ev.len();
                tokio::time::sleep(Duration::from_millis(3)).await;
            }
            reads
        })
    };

    let (_, reads) = tokio::join!(write_handle, read_handle);
    let reads = reads.unwrap();
    // WAL mode allows reads from a separate connection while writes are in progress.
    assert!(reads > 0);

    db_writer.close().await.unwrap();
    db_reader.close().await.unwrap();
}

#[tokio::test]
async fn test_backup_and_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.event_repo()
        .append("goal-1", "start", "{}")
        .await
        .unwrap();

    let backup_path = dir.path().join("backup.db");
    db.backup_to(&backup_path).await.unwrap();

    let restored = DbHandle::open(&backup_path).await.unwrap();
    let goal = restored.goal_repo().get("goal-1").await.unwrap();
    assert!(goal.is_some());
    let events = restored
        .event_repo()
        .get_by_goal("goal-1", None, None)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);

    db.close().await.unwrap();
    restored.close().await.unwrap();
}

#[tokio::test]
async fn test_migration_idempotency() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    // Simulate re-opening (re-applying migrations).
    let db2 = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    assert!(db2.goal_repo().get("goal-1").await.unwrap().is_some());

    db.close().await.unwrap();
    db2.close().await.unwrap();
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
