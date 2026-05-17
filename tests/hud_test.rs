use std::path::PathBuf;

use omk::runtime::events::{Event, EventKind, EventWriter, RunId};
use omk::runtime::state::{TaskStatus, TeamState};
use omk::runtime::watchdog::Watchdog;
use omk::vis::event_stream::EventStream;
use omk::vis::hud::HudState;

async fn setup_mock_team_state(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = dir.path().join("team").join(name);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    let team_state = TeamState::new(name, "fix all errors", &state_dir, 2, "coder");
    team_state.save().await.unwrap();

    // Create workers
    for i in 0..2 {
        let worker_dir = state_dir.join("workers").join(format!("worker-{i}"));
        tokio::fs::create_dir_all(&worker_dir).await.unwrap();

        let spec = omk::runtime::worker::WorkerSpec {
            name: format!("worker-{i}"),
            role: "coder".to_string(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
            project_dir: None,
            external_tools: None,
            approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
            approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
        };
        spec.save().await.unwrap();

        // Write a heartbeat
        let heartbeat = serde_json::json!({
            "status": "alive",
            "ts": chrono::Utc::now().to_rfc3339()
        });
        tokio::fs::write(worker_dir.join("heartbeat.json"), heartbeat.to_string())
            .await
            .unwrap();
    }

    (dir, state_dir)
}

#[tokio::test]
async fn test_event_stream_poll_reads_incremental_events() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("events.jsonl");

    let mut stream = EventStream::new(&path);

    // Initial poll on non-existent file
    let events = stream.poll().await.unwrap();
    assert!(events.is_empty());

    // Write first event
    let writer = EventWriter::new(&path);
    let e1 = Event::new(RunId("run-1".to_string()), EventKind::RunStarted);
    writer.append(&e1).await.unwrap();

    let events = stream.poll().await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, EventKind::RunStarted));

    // Write second and third events
    let e2 = Event::new(RunId("run-1".to_string()), EventKind::WorkerStarted).with_actor("w1");
    let e3 = Event::new(RunId("run-1".to_string()), EventKind::TaskStarted)
        .with_payload(serde_json::json!({ "task_id": "task-1" }))
        .unwrap();
    writer.append(&e2).await.unwrap();
    writer.append(&e3).await.unwrap();

    let events = stream.poll().await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].kind, EventKind::WorkerStarted));
    assert!(matches!(events[1].kind, EventKind::TaskStarted));

    // No new events
    let events = stream.poll().await.unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn test_hud_state_refresh_and_render() {
    let (_dir, state_dir) = setup_mock_team_state("hud-test").await;
    let events_path = state_dir.join("events.jsonl");

    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();

    let mut hud = HudState::new("hud-test", "hud-test");

    // Initial refresh — no events yet
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    // Write some events
    let writer = EventWriter::new(&events_path);
    let run_started = Event::new(RunId("hud-test".to_string()), EventKind::RunStarted);
    writer.append(&run_started).await.unwrap();

    let task_started = Event::new(RunId("hud-test".to_string()), EventKind::TaskStarted)
        .with_payload(serde_json::json!({ "task_id": "task-1" }))
        .unwrap();
    writer.append(&task_started).await.unwrap();

    let task_completed = Event::new(RunId("hud-test".to_string()), EventKind::TaskCompleted)
        .with_payload(serde_json::json!({ "task_id": "task-1" }))
        .unwrap();
    writer.append(&task_completed).await.unwrap();

    // Refresh again
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let text = hud.render_text();
    assert!(text.contains("team: hud-test"));
    assert!(text.contains("run: hud-test"));
    assert!(text.contains("Workers: 2 total"));
    assert!(text.contains("Events:  3"));

    // JSON render should succeed
    let json = hud.render_json().unwrap();
    assert!(json.contains("\"team_name\": \"hud-test\""));
}

#[tokio::test]
async fn test_hud_state_with_team_tasks() {
    let (_dir, state_dir) = setup_mock_team_state("hud-tasks").await;

    // Update team state with tasks
    let mut team_state = TeamState::load(&state_dir).await.unwrap();
    team_state.tasks.push(omk::runtime::state::Task {
        id: "t1".to_string(),
        description: "task 1".to_string(),
        assigned_to: Some("worker-0".to_string()),
        status: TaskStatus::Done,
        created_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
    });
    team_state.tasks.push(omk::runtime::state::Task {
        id: "t2".to_string(),
        description: "task 2".to_string(),
        assigned_to: Some("worker-1".to_string()),
        status: TaskStatus::InProgress,
        created_at: chrono::Utc::now(),
        completed_at: None,
    });
    team_state.tasks.push(omk::runtime::state::Task {
        id: "t3".to_string(),
        description: "task 3".to_string(),
        assigned_to: None,
        status: TaskStatus::Pending,
        created_at: chrono::Utc::now(),
        completed_at: None,
    });
    team_state.save().await.unwrap();

    let events_path = state_dir.join("events.jsonl");
    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();

    let mut hud = HudState::new("hud-tasks", "hud-tasks");
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let text = hud.render_text();
    assert!(text.contains("Tasks:   3 total | 1 completed | 1 running | 1 pending"));
}

#[tokio::test]
async fn test_hud_worker_display_computation() {
    let (_dir, state_dir) = setup_mock_team_state("hud-display").await;
    let events_path = state_dir.join("events.jsonl");

    let writer = EventWriter::new(&events_path);
    let run_id = RunId("hud-display".to_string());

    writer
        .append(&Event::new(run_id.clone(), EventKind::RunStarted))
        .await
        .unwrap();
    writer
        .append(&Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor("worker-0"))
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::TaskClaimed)
                .with_actor("worker-0")
                .with_payload(serde_json::json!({ "task_id": "task-1" }))
                .unwrap(),
        )
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::RetryScheduled)
                .with_actor("scheduler")
                .with_payload(serde_json::json!({ "task_id": "task-1", "attempt": 2 }))
                .unwrap(),
        )
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::GatePassed)
                .with_payload(
                    serde_json::json!({ "gate_id": "g1", "name": "fmt", "required": true }),
                )
                .unwrap(),
        )
        .await
        .unwrap();

    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::new(omk::runtime::watchdog::WatchdogConfig {
        ..Default::default()
    });
    let mut hud = HudState::new("hud-display", "hud-display");
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let displays = hud.worker_displays();
    assert_eq!(displays.len(), 2);

    let w0 = displays.iter().find(|d| d.name == "worker-0").unwrap();
    assert_eq!(w0.status, "Busy"); // Healthy + has task
    assert_eq!(w0.current_task_id, Some("task-1".to_string()));
    assert_eq!(w0.retry_count, 1);
    assert_eq!(w0.gate_status, "passed");

    let w1 = displays.iter().find(|d| d.name == "worker-1").unwrap();
    assert_eq!(w1.status, "Ready"); // Healthy + no task
    assert!(w1.current_task_id.is_none());
    assert_eq!(w1.retry_count, 0);
    assert_eq!(w1.gate_status, "passed"); // Global gate status
}

#[tokio::test]
async fn test_hud_worker_display_stalled_and_dead() {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = dir.path().join("team").join("stalled-test");
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    let team_state = TeamState::new("stalled-test", "test", &state_dir, 1, "coder");
    team_state.save().await.unwrap();

    // Create worker without heartbeat
    let worker_dir = state_dir.join("workers").join("worker-0");
    tokio::fs::create_dir_all(&worker_dir).await.unwrap();
    let spec = omk::runtime::worker::WorkerSpec {
        name: "worker-0".to_string(),
        role: "coder".to_string(),
        inbox: worker_dir.join("inbox.jsonl"),
        outbox: worker_dir.join("outbox.jsonl"),
        heartbeat: worker_dir.join("heartbeat.json"),
        project_dir: None,
        external_tools: None,
        approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
        approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
    };
    spec.save().await.unwrap();

    let events_path = state_dir.join("events.jsonl");
    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();
    let mut hud = HudState::new("stalled-test", "stalled-test");
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let displays = hud.worker_displays();
    assert_eq!(displays.len(), 1);
    assert_eq!(displays[0].status, "Dead");
    assert_eq!(displays[0].heartbeat_age_secs, -1);
    assert!(displays[0].current_task_id.is_none());
}

#[tokio::test]
async fn test_hud_worker_display_heartbeat_from_event() {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = dir.path().join("team").join("hb-event-test");
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    let team_state = TeamState::new("hb-event-test", "test", &state_dir, 1, "coder");
    team_state.save().await.unwrap();

    // Create worker without heartbeat file
    let worker_dir = state_dir.join("workers").join("worker-0");
    tokio::fs::create_dir_all(&worker_dir).await.unwrap();
    let spec = omk::runtime::worker::WorkerSpec {
        name: "worker-0".to_string(),
        role: "coder".to_string(),
        inbox: worker_dir.join("inbox.jsonl"),
        outbox: worker_dir.join("outbox.jsonl"),
        heartbeat: worker_dir.join("heartbeat.json"),
        project_dir: None,
        external_tools: None,
        approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
        approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
    };
    spec.save().await.unwrap();

    // Write a WorkerHeartbeat event instead
    let events_path = state_dir.join("events.jsonl");
    let writer = EventWriter::new(&events_path);
    let run_id = RunId("hb-event-test".to_string());
    writer
        .append(&Event::new(run_id.clone(), EventKind::RunStarted))
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::WorkerHeartbeat)
                .with_actor("worker-0")
                .with_payload(
                    serde_json::json!({ "worker_id": "worker-0", "timestamp": chrono::Utc::now() }),
                )
                .unwrap(),
        )
        .await
        .unwrap();

    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();
    let mut hud = HudState::new("hb-event-test", "hb-event-test");
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let displays = hud.worker_displays();
    assert_eq!(displays.len(), 1);
    assert_eq!(displays[0].status, "Dead"); // No heartbeat file = Dead per watchdog
                                            // But heartbeat_age_secs should be computed from the WorkerHeartbeat event
    assert!(displays[0].heartbeat_age_secs >= 0);
}

#[tokio::test]
async fn test_hud_refresh_reads_all_events_from_file() {
    let (_dir, state_dir) = setup_mock_team_state("hud-all-events").await;
    let events_path = state_dir.join("events.jsonl");

    // Write events directly without using EventStream
    let writer = EventWriter::new(&events_path);
    let run_id = RunId("hud-all-events".to_string());
    for i in 0..5 {
        let event =
            Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor(format!("worker-{i}"));
        writer.append(&event).await.unwrap();
    }

    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();
    let mut hud = HudState::new("hud-all-events", "hud-all-events");

    // First refresh should load all events
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    assert_eq!(hud.events.len(), 5);

    // Add more events directly
    for i in 5..8 {
        let event =
            Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor(format!("worker-{i}"));
        writer.append(&event).await.unwrap();
    }

    // Second refresh should pick up the new events
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    assert_eq!(hud.events.len(), 8);
}

#[tokio::test]
async fn test_hud_state_json_exposes_latest_gate_and_proof_status() {
    let (_dir, state_dir) = setup_mock_team_state("hud-json-proof-gate").await;
    let events_path = state_dir.join("events.jsonl");
    let writer = EventWriter::new(&events_path);
    let run_id = RunId("hud-json-proof-gate".to_string());

    writer
        .append(
            &Event::new(run_id.clone(), EventKind::GateFailed)
                .with_payload(
                    serde_json::json!({ "gate_id": "g1", "name": "fmt", "required": true }),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::GateFailed)
                .with_payload(
                    serde_json::json!({ "gate_id": "g2", "name": "lint", "required": true }),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::ProofWritten)
                .with_payload(serde_json::json!({ "proof_path": "/tmp/p1", "status": "failed" }))
                .unwrap(),
        )
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::ProofWritten)
                .with_payload(serde_json::json!({ "proof_path": "/tmp/p2", "status": "ready" }))
                .unwrap(),
        )
        .await
        .unwrap();

    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();
    let mut hud = HudState::new("hud-json-proof-gate", "hud-json-proof-gate");
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let json: serde_json::Value = serde_json::from_str(&hud.render_json().unwrap()).unwrap();
    assert_eq!(json["latest_failed_gate"], "lint");
    assert_eq!(json["proof_status"], "ready");
}

#[tokio::test]
async fn test_hud_state_text_shows_latest_gate_and_proof_status() {
    let (_dir, state_dir) = setup_mock_team_state("hud-text-proof-gate").await;
    let events_path = state_dir.join("events.jsonl");
    let writer = EventWriter::new(&events_path);
    let run_id = RunId("hud-text-proof-gate".to_string());

    writer
        .append(
            &Event::new(run_id.clone(), EventKind::GateFailed)
                .with_payload(
                    serde_json::json!({ "gate_id": "g1", "name": "unit", "required": true }),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    writer
        .append(
            &Event::new(run_id.clone(), EventKind::ProofWritten)
                .with_payload(serde_json::json!({ "proof_path": "/tmp/p", "status": "failed" }))
                .unwrap(),
        )
        .await
        .unwrap();

    let mut event_stream = EventStream::new(&events_path);
    let watchdog = Watchdog::with_defaults();
    let mut hud = HudState::new("hud-text-proof-gate", "hud-text-proof-gate");
    hud.refresh(&mut event_stream, &watchdog, &state_dir)
        .await
        .unwrap();

    let text = hud.render_text();
    assert!(text.contains("Gate:    unit (failed)"));
    assert!(text.contains("Proof:   failed"));
}
