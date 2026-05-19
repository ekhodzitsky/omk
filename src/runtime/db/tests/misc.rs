use super::test_goal;
use crate::runtime::db::handle::DbHandle;
use crate::runtime::db::repo::{
    artifact::ArtifactRepo, budget::BudgetRepo, event::EventRepo, goal::GoalRepo, proof::ProofRepo,
    task::TaskRepo,
};
use crate::runtime::db::types::{BudgetCheckpoint, ProofRecord};

#[tokio::test]
async fn test_cascading_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = DbHandle::open(&path).await.unwrap();

    db.goal_repo().create(&test_goal()).await.unwrap();
    db.task_repo()
        .create_batch("goal-1", &[super::test_task("t1", "goal-1")])
        .await
        .unwrap();
    db.event_repo()
        .append("goal-1", "start", "{}")
        .await
        .unwrap();
    db.proof_repo()
        .upsert(&ProofRecord {
            goal_id: "goal-1".to_string(),
            version: 1,
            status: "pending".to_string(),
            readiness: "not_ready".to_string(),
            summary: "Proof pending".to_string(),
            task_graph_summary: None,
            changed_files: None,
            commits: None,
            git: None,
            gates: None,
            gates_passed: 0,
            gates_total: 0,
            post_mutation_gates_ran: false,
            known_gaps: None,
            human_decisions_required: None,
            recovery_status: None,
            generated_at: 1,
        })
        .await
        .unwrap();
    db.budget_repo()
        .append_checkpoint(&BudgetCheckpoint {
            checkpoint_id: None,
            goal_id: "goal-1".to_string(),
            version: 1,
            label: "usd".to_string(),
            status: "active".to_string(),
            phase: "planning".to_string(),
            recorded_at: 1,
            budget_time: None,
            total_budget_secs: Some(3600),
            elapsed_since_created_secs: 0,
            remaining_budget_secs: Some(3600),
            budget_tokens: Some(1_000_000),
            used_tokens: 100,
            remaining_budget_tokens: Some(999_900),
            budget_usd: Some(1000),
            estimated_cost_usd: 100,
            remaining_budget_usd: Some(900),
            limit_value: Some(1000),
            used_value: Some(100),
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
