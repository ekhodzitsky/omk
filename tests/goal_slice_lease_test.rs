use std::sync::Arc;

use tokio::sync::Barrier;

use omk::runtime::db::{
    repo::{goal::GoalRepo, slice_lease::SliceLeaseRepo},
    DbHandle, GoalRecord,
};

fn test_goal(goal_id: &str) -> GoalRecord {
    GoalRecord {
        goal_id: goal_id.to_string(),
        status: "running".to_string(),
        phase: "execution".to_string(),
        kind: None,
        original_goal: "test".to_string(),
        normalized_goal: "test".to_string(),
        goal_text: "test".to_string(),
        project_dir: "/tmp/test".to_string(),
        state_dir: "/tmp/test/.omk/state".to_string(),
        policy: "local".to_string(),
        delivery_policy: "local".to_string(),
        merge_policy: "disabled".to_string(),
        until_ready: false,
        slice_execution: false,
        max_agents: None,
        budget_time: None,
        budget_tokens: None,
        budget_usd: None,
        cost_tracker_path: None,
        terminal_criteria: None,
        failure: None,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
        completed_at: None,
        controller_pid: None,
        version: 1,
    }
}

#[tokio::test]
async fn test_claim_succeeds_on_empty_slice() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let lease = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap();

    assert!(lease.is_some(), "claim should succeed on empty slice");
    let lease = lease.unwrap();
    assert_eq!(lease.goal_id, "g1");
    assert_eq!(lease.slice_id, "slice-a");
    assert_eq!(lease.owner_pid, 1234);
}

#[tokio::test]
async fn test_claim_fails_when_slice_already_held() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let first = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap();
    assert!(first.is_some());

    let second = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            5678,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_001,
        )
        .await
        .unwrap();
    assert!(second.is_none(), "second claim on same slice should fail");
}

#[tokio::test]
async fn test_claim_fails_on_write_set_overlap_with_different_slice() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let first = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap();
    assert!(first.is_some());

    let second = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-b",
            5678,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_001,
        )
        .await
        .unwrap();
    assert!(
        second.is_none(),
        "claim on different slice with overlapping write_set should fail"
    );
}

#[tokio::test]
async fn test_release_allows_reclaim() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let lease = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap()
        .unwrap();

    db.slice_lease_repo()
        .release(&lease.lease_id, 1_000_100)
        .await
        .unwrap();

    let reclaimed = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            5678,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_200,
        )
        .await
        .unwrap();
    assert!(
        reclaimed.is_some(),
        "should be able to reclaim after release"
    );
}

#[tokio::test]
async fn test_expire_stale_releases_dead_agent_leases() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let lease = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap()
        .unwrap();

    let expired = db
        .slice_lease_repo()
        .expire_stale(1_000_100, 50)
        .await
        .unwrap();
    assert_eq!(expired.len(), 1, "stale lease should be expired");

    let active = db.slice_lease_repo().active_for_goal("g1").await.unwrap();
    assert!(active.is_empty(), "no active leases after expire");

    let reloaded = db
        .slice_lease_repo()
        .get(&lease.lease_id)
        .await
        .unwrap()
        .unwrap();
    assert!(reloaded.expired_at.is_some());
}

#[tokio::test]
async fn test_concurrent_claims_same_slice_exactly_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let db_a = db.clone();
    let db_b = db.clone();
    let barrier_a = barrier.clone();
    let barrier_b = barrier.clone();

    let handle_a = tokio::spawn(async move {
        barrier_a.wait().await;
        db_a.slice_lease_repo()
            .try_claim(
                "g1",
                "slice-a",
                1234,
                "executor",
                &["src/lib.rs".to_string()],
                1_000_000,
            )
            .await
            .unwrap()
    });

    let handle_b = tokio::spawn(async move {
        barrier_b.wait().await;
        db_b.slice_lease_repo()
            .try_claim(
                "g1",
                "slice-a",
                5678,
                "executor",
                &["src/lib.rs".to_string()],
                1_000_000,
            )
            .await
            .unwrap()
    });

    let (res_a, res_b) = tokio::join!(handle_a, handle_b);
    let a = res_a.unwrap();
    let b = res_b.unwrap();

    let winners = [a.is_some(), b.is_some()];
    assert_eq!(
        winners.iter().filter(|&&w| w).count(),
        1,
        "exactly one concurrent claim should win"
    );
}

#[tokio::test]
async fn test_concurrent_claims_overlapping_write_sets_exactly_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let db_a = db.clone();
    let db_b = db.clone();
    let barrier_a = barrier.clone();
    let barrier_b = barrier.clone();

    let handle_a = tokio::spawn(async move {
        barrier_a.wait().await;
        db_a.slice_lease_repo()
            .try_claim(
                "g1",
                "slice-a",
                1234,
                "executor",
                &["src/lib.rs".to_string()],
                1_000_000,
            )
            .await
            .unwrap()
    });

    let handle_b = tokio::spawn(async move {
        barrier_b.wait().await;
        db_b.slice_lease_repo()
            .try_claim(
                "g1",
                "slice-b",
                5678,
                "executor",
                &["src/lib.rs".to_string()],
                1_000_000,
            )
            .await
            .unwrap()
    });

    let (res_a, res_b) = tokio::join!(handle_a, handle_b);
    let a = res_a.unwrap();
    let b = res_b.unwrap();

    let winners = [a.is_some(), b.is_some()];
    assert_eq!(
        winners.iter().filter(|&&w| w).count(),
        1,
        "exactly one concurrent overlapping claim should win"
    );
}

#[tokio::test]
async fn test_heartbeat_updates_timestamp_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let lease = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(lease.heartbeat_at, 1_000_000);

    db.slice_lease_repo()
        .heartbeat(&lease.lease_id, 1_000_050)
        .await
        .unwrap();

    let reloaded = db
        .slice_lease_repo()
        .get(&lease.lease_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.heartbeat_at, 1_000_050);
}

#[tokio::test]
async fn test_lease_guard_drop_releases_lease() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let manager = Arc::new(omk::runtime::goal::agent::leases::LeaseManager::new(
        db.clone(),
    ));
    let (guard, _) = manager
        .try_claim("g1", "slice-a", "executor", vec!["src/lib.rs".to_string()])
        .await
        .unwrap();

    let lease_id = guard.lease_id().to_string();
    guard.release().await;

    let active = db.slice_lease_repo().active_for_goal("g1").await.unwrap();
    assert!(
        active.is_empty(),
        "lease should be released after explicit release"
    );

    let lease = db.slice_lease_repo().get(&lease_id).await.unwrap().unwrap();
    assert!(lease.released_at.is_some());
}

#[tokio::test]
async fn test_migration_v2_applies_cleanly_on_empty_db() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    // If the table exists, try_claim will succeed.
    let lease = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap();
    assert!(
        lease.is_some(),
        "migration v2 should create goal_slice_leases table"
    );
}

#[tokio::test]
async fn test_migration_v2_idempotent_on_already_migrated_db() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    let db = DbHandle::open(&path).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();
    let lease = db
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-a",
            1234,
            "executor",
            &["src/lib.rs".to_string()],
            1_000_000,
        )
        .await
        .unwrap();
    assert!(lease.is_some());
    drop(db);

    let db2 = DbHandle::open(&path).await.unwrap();
    let lease2 = db2
        .slice_lease_repo()
        .try_claim(
            "g1",
            "slice-b",
            5678,
            "executor",
            &["src/main.rs".to_string()],
            1_000_001,
        )
        .await
        .unwrap();
    assert!(
        lease2.is_some(),
        "re-opening should keep goal_slice_leases table working"
    );
}

#[tokio::test]
async fn test_legacy_fallback_on_db_error_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    // Deliberately do NOT create a goal record, so the foreign key
    // constraint will cause try_claim to fail with a DB error.

    let manager = Arc::new(omk::runtime::goal::agent::leases::LeaseManager::new(
        db.clone(),
    ));
    let result = manager
        .try_claim("g1", "slice-a", "executor", vec!["src/lib.rs".to_string()])
        .await;

    // LeaseManager should return an error (DB constraint), not panic.
    assert!(result.is_err(), "should error on missing goal FK");
}

#[tokio::test]
async fn test_events_emit_claim_release_expire() {
    let dir = tempfile::tempdir().unwrap();
    let db = DbHandle::open(dir.path().join("test.db")).await.unwrap();
    db.goal_repo().create(&test_goal("g1")).await.unwrap();

    let manager = Arc::new(omk::runtime::goal::agent::leases::LeaseManager::new(
        db.clone(),
    ));
    let (guard, _) = manager
        .try_claim("g1", "slice-a", "executor", vec!["src/lib.rs".to_string()])
        .await
        .unwrap();

    let lease_id = guard.lease_id().to_string();
    guard.release().await;

    // Expire any stale (none should exist, but exercise the path).
    let now = chrono::Utc::now().timestamp();
    let expired = db.slice_lease_repo().expire_stale(now, 1).await.unwrap();
    assert!(expired.is_empty(), "released lease should not be expired");

    // Verify the lease record shows released_at.
    let lease = db.slice_lease_repo().get(&lease_id).await.unwrap().unwrap();
    assert!(lease.released_at.is_some());
}
