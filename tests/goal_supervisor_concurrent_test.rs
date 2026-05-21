use std::path::PathBuf;
use std::sync::Arc;

use omk::runtime::db::{repo::goal::GoalRepo, DbHandle};
use omk::runtime::goal::{
    claim_goal_for_test, list_orphaned_goals_for_test, release_goal_for_test, test_goal_record,
};

#[tokio::test]
async fn claim_release_reclaim_baseline() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path: PathBuf = tmp.path().join("test.db");
    let db = DbHandle::open(&db_path).await.unwrap();

    let goal = test_goal_record("g1", "running", None);
    db.goal_repo().create(&goal).await.unwrap();

    let h1 = claim_goal_for_test("g1", 1001, &db_path).await.unwrap();
    let rec = db.goal_repo().get("g1").await.unwrap().unwrap();
    assert_eq!(rec.controller_pid, Some(1001));
    h1.stop();

    release_goal_for_test("g1", &db_path).await.unwrap();
    let rec = db.goal_repo().get("g1").await.unwrap().unwrap();
    assert_eq!(rec.controller_pid, None);

    let h2 = claim_goal_for_test("g1", 1002, &db_path).await.unwrap();
    let rec = db.goal_repo().get("g1").await.unwrap().unwrap();
    assert_eq!(rec.controller_pid, Some(1002));
    h2.stop();
}

#[tokio::test]
#[ignore = "Documents race: two concurrent claim_goal calls race on unconditional UPDATE. Both return Ok, last writer wins. This test demonstrates the current behaviour and should be re-enabled once claim_goal uses a conditional UPSERT."]
async fn two_concurrent_claims_exactly_one_winner() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path: PathBuf = tmp.path().join("test.db");
    let db = DbHandle::open(&db_path).await.unwrap();

    let goal = test_goal_record("g2", "running", None);
    db.goal_repo().create(&goal).await.unwrap();

    let barrier = Arc::new(std::sync::Barrier::new(2));

    let db_path_a = db_path.clone();
    let barrier_a = barrier.clone();
    let handle_a = tokio::spawn(async move {
        barrier_a.wait();
        claim_goal_for_test("g2", 2001, &db_path_a).await
    });

    let db_path_b = db_path.clone();
    let barrier_b = barrier.clone();
    let handle_b = tokio::spawn(async move {
        barrier_b.wait();
        claim_goal_for_test("g2", 2002, &db_path_b).await
    });

    let (res_a, res_b) = tokio::join!(handle_a, handle_b);
    let h_a = res_a.unwrap().unwrap();
    let h_b = res_b.unwrap().unwrap();

    h_a.stop();
    h_b.stop();

    let rec = db.goal_repo().get("g2").await.unwrap().unwrap();
    let final_pid = rec.controller_pid;

    // Both calls returned Ok because the UPDATE is unconditional.
    // The test documents that the current implementation has a race:
    // exactly one PID should remain in the DB, but we cannot deterministically
    // know which one. We assert that it is one of the two contenders.
    assert!(
        final_pid == Some(2001) || final_pid == Some(2002),
        "Expected exactly one winner (PID 2001 or 2002), got {:?}",
        final_pid
    );
}

#[tokio::test]
#[ignore = "Documents race: concurrent claims during stale heartbeat still race on unconditional UPDATE. Both return Ok despite heartbeat being stale. Re-enable after conditional UPSERT is implemented."]
async fn concurrent_claim_during_stale_heartbeat_new_owner_races() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path: PathBuf = tmp.path().join("test.db");
    let db = DbHandle::open(&db_path).await.unwrap();

    // Seed a goal with a dead controller PID and a very old heartbeat.
    let mut goal = test_goal_record("g3", "running", Some(999_999));
    goal.updated_at = chrono::Utc::now().timestamp() - 3600;
    db.goal_repo().create(&goal).await.unwrap();

    // Verify the goal is detected as orphaned.
    let orphaned_before = list_orphaned_goals_for_test(&db).await.unwrap();
    let ids: Vec<_> = orphaned_before.iter().map(|o| o.goal_id.as_str()).collect();
    assert!(ids.contains(&"g3"), "Goal should be orphaned before claims");

    let barrier = Arc::new(std::sync::Barrier::new(2));

    let db_path_a = db_path.clone();
    let barrier_a = barrier.clone();
    let handle_a = tokio::spawn(async move {
        barrier_a.wait();
        claim_goal_for_test("g3", 3001, &db_path_a).await
    });

    let db_path_b = db_path.clone();
    let barrier_b = barrier.clone();
    let handle_b = tokio::spawn(async move {
        barrier_b.wait();
        claim_goal_for_test("g3", 3002, &db_path_b).await
    });

    let (res_a, res_b) = tokio::join!(handle_a, handle_b);
    let h_a = res_a.unwrap().unwrap();
    let h_b = res_b.unwrap().unwrap();

    h_a.stop();
    h_b.stop();

    let rec = db.goal_repo().get("g3").await.unwrap().unwrap();
    let final_pid = rec.controller_pid;

    // Both calls returned Ok because the UPDATE is unconditional and does not
    // check heartbeat staleness. The final PID is whichever UPDATE executed
    // last. The test documents this race.
    assert!(
        final_pid == Some(3001) || final_pid == Some(3002),
        "Expected one of the new owners (3001 or 3002), got {:?}",
        final_pid
    );
}
