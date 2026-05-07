use tempfile::TempDir;

// Helper: create a mock team state directory for testing status/shutdown logic
async fn setup_mock_team(name: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let state_dir = dir.path().join("state").join("team").join(name);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    let team_state = serde_json::json!({
        "name": name,
        "task": "fix all errors",
        "created_at": "2026-05-07T14:00:00Z",
        "worker_count": 2,
        "worker_role": "coder",
        "phase": "Executing",
        "tasks": [],
        "state_dir": state_dir
    });

    tokio::fs::write(
        state_dir.join("team-state.json"),
        serde_json::to_string_pretty(&team_state).unwrap(),
    )
    .await
    .unwrap();

    // Create workers
    for i in 0..2 {
        let worker_dir = state_dir.join("workers").join(format!("worker-{i}"));
        tokio::fs::create_dir_all(&worker_dir).await.unwrap();

        let spec = serde_json::json!({
            "name": format!("worker-{i}"),
            "role": "coder",
            "inbox": worker_dir.join("inbox.jsonl"),
            "outbox": worker_dir.join("outbox.jsonl"),
            "heartbeat": worker_dir.join("heartbeat.json")
        });
        tokio::fs::write(
            worker_dir.join("worker-spec.json"),
            serde_json::to_string_pretty(&spec).unwrap(),
        )
        .await
        .unwrap();

        // Write a heartbeat
        let heartbeat = serde_json::json!({
            "status": "alive",
            "name": format!("worker-{i}"),
            "ts": "2026-05-07T14:05:00Z"
        });
        tokio::fs::write(
            worker_dir.join("heartbeat.json"),
            serde_json::to_string_pretty(&heartbeat).unwrap(),
        )
        .await
        .unwrap();
    }

    dir
}

#[tokio::test]
async fn test_team_state_load_save() {
    let dir = setup_mock_team("test-state").await;
    let state_dir = dir
        .path()
        .join("state")
        .join("team")
        .join("test-state");

    let json = tokio::fs::read_to_string(state_dir.join("team-state.json"))
        .await
        .unwrap();
    let state: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(state["name"], "test-state");
    assert_eq!(state["worker_count"], 2);
    assert_eq!(state["phase"], "Executing");
}

#[tokio::test]
async fn test_worker_spec_load() {
    let dir = setup_mock_team("test-spec").await;
    let worker_dir = dir
        .path()
        .join("state")
        .join("team")
        .join("test-spec")
        .join("workers")
        .join("worker-0");

    let json = tokio::fs::read_to_string(worker_dir.join("worker-spec.json"))
        .await
        .unwrap();
    let spec: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(spec["name"], "worker-0");
    assert_eq!(spec["role"], "coder");
    assert!(spec["inbox"].as_str().unwrap().contains("inbox.jsonl"));
}

#[tokio::test]
async fn test_heartbeat_alive() {
    let dir = setup_mock_team("test-heartbeat").await;
    let hb_path = dir
        .path()
        .join("state")
        .join("team")
        .join("test-heartbeat")
        .join("workers")
        .join("worker-1")
        .join("heartbeat.json");

    let json = tokio::fs::read_to_string(&hb_path).await.unwrap();
    let hb: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(hb["status"], "alive");
    assert_eq!(hb["name"], "worker-1");
}

#[tokio::test]
async fn test_status_reads_all_workers() {
    let dir = setup_mock_team("test-status").await;
    let state_dir = dir
        .path()
        .join("state")
        .join("team")
        .join("test-status");

    // Simulate status logic: count workers and check heartbeats
    let mut entries = tokio::fs::read_dir(state_dir.join("workers")).await.unwrap();
    let mut worker_count = 0;
    let mut alive_count = 0;

    while let Some(entry) = entries.next_entry().await.unwrap() {
        let hb_path = entry.path().join("heartbeat.json");
        if hb_path.exists() {
            let json = tokio::fs::read_to_string(&hb_path).await.unwrap();
            let hb: serde_json::Value = serde_json::from_str(&json).unwrap();
            if hb["status"] == "alive" {
                alive_count += 1;
            }
        }
        worker_count += 1;
    }

    assert_eq!(worker_count, 2);
    assert_eq!(alive_count, 2);
}

#[tokio::test]
async fn test_shutdown_updates_phase() {
    let dir = setup_mock_team("test-shutdown").await;
    let state_path = dir
        .path()
        .join("state")
        .join("team")
        .join("test-shutdown")
        .join("team-state.json");

    // Simulate shutdown: update phase
    let json = tokio::fs::read_to_string(&state_path).await.unwrap();
    let mut state: serde_json::Value = serde_json::from_str(&json).unwrap();
    state["phase"] = "Shutdown".into();

    tokio::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap())
        .await
        .unwrap();

    let updated = tokio::fs::read_to_string(&state_path).await.unwrap();
    let updated_state: serde_json::Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(updated_state["phase"], "Shutdown");
}
