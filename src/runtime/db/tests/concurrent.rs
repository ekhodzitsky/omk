use std::time::Duration;

use super::test_goal;
use crate::runtime::db::handle::DbHandle;
use crate::runtime::db::repo::{event::EventRepo, goal::GoalRepo};

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
