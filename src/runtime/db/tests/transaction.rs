use super::test_goal;
use crate::runtime::db::handle::DbHandle;
use crate::runtime::db::repo::goal::GoalRepo;

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
