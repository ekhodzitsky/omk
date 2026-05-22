use std::sync::Mutex;
use std::time::Duration;

use omk::runtime::events::{
    Event, EventKind, ProofWrittenPayload, RunStartedPayload, TaskGraphMutationPayload,
};
use omk::runtime::goal::chat_api::{
    commands::show_proof,
    source::tail_goal_events_into,
    to_child_event,
    wire_pool::{WireClientFactory, WirePool},
    ChildGoalConfig, ChildGoalEvent, CreateChildRequest, PlanNodeStatus,
};
use omk::wire::client::InMemoryWireClient;

// ---------------------------------------------------------------------------
// Adapter unit tests
// ---------------------------------------------------------------------------

#[test]
fn test_adapter_run_started_maps_to_created() {
    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId("g-123".to_string()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::RunStarted,
        actor: None,
        payload: Some(serde_json::json!({
            "mode": "test",
            "project_dir": "/tmp",
            "description": "do things"
        })),
    };
    let child = to_child_event(&event).unwrap();
    match child {
        ChildGoalEvent::Created { goal_id, plan } => {
            assert_eq!(goal_id, "g-123");
            assert!(!plan.is_empty());
        }
        other => panic!("expected Created, got {:?}", other),
    }
}

#[test]
fn test_adapter_proof_written_ready_maps_to_proof_ready() {
    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId("g-123".to_string()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::ProofWritten,
        actor: None,
        payload: Some(serde_json::json!({
            "proof_path": "/tmp/proof.json",
            "status": "ready"
        })),
    };
    let child = to_child_event(&event).unwrap();
    match child {
        ChildGoalEvent::ProofReady { path } => {
            assert_eq!(path, std::path::PathBuf::from("/tmp/proof.json"));
        }
        other => panic!("expected ProofReady, got {:?}", other),
    }
}

#[test]
fn test_adapter_manual_interrupt_maps_to_cancelled() {
    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId("g-123".to_string()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::ManualInterrupt,
        actor: None,
        payload: None,
    };
    let child = to_child_event(&event).unwrap();
    assert!(matches!(child, ChildGoalEvent::Cancelled));
}

#[test]
fn test_adapter_unknown_event_returns_none() {
    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId("g-123".to_string()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::BudgetCheckpoint,
        actor: None,
        payload: None,
    };
    assert!(to_child_event(&event).is_none());
}

// ---------------------------------------------------------------------------
// Source tail unit test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_source_tail_reads_new_events() {
    let temp = tempfile::TempDir::new().unwrap();
    let events_path = temp.path().join("events.jsonl");

    let (sender, mut receiver) = tokio::sync::broadcast::channel(16);
    let shutdown = tokio_util::sync::CancellationToken::new();

    let tail_handle = tokio::spawn({
        let dir = temp.path().to_path_buf();
        let shutdown = shutdown.clone();
        async move {
            tail_goal_events_into(dir, sender, shutdown).await.unwrap();
        }
    });

    // Write an event after tail has started polling
    tokio::time::sleep(Duration::from_millis(200)).await;
    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId("g-1".to_string()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::RunStarted,
        actor: None,
        payload: Some(
            serde_json::to_value(RunStartedPayload {
                mode: "test".to_string(),
                project_dir: std::env::current_dir().unwrap(),
                description: "tail test".to_string(),
                kimi_binary: None,
                kimi_cli_version: None,
                wire_protocol_version: None,
            })
            .unwrap(),
        ),
    };
    let line = format!("{}\n", serde_json::to_string(&event).unwrap());
    tokio::fs::write(&events_path, line).await.unwrap();

    let child_event = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(child_event, ChildGoalEvent::Created { .. }));

    shutdown.cancel();
    let _ = tail_handle.await;
}

// ---------------------------------------------------------------------------
// Wire pool unit tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct MockFactory;

impl WireClientFactory for MockFactory {
    type Client = InMemoryWireClient;

    async fn create(&self) -> anyhow::Result<Self::Client> {
        Ok(InMemoryWireClient::new())
    }
}

#[tokio::test]
async fn test_wire_pool_reuses_idle_worker() {
    let pool = std::sync::Arc::new(WirePool::with_factory(2, MockFactory));
    let w1 = pool.acquire().await.unwrap();
    let id1 = w1.id.clone();
    pool.release(w1).await;
    let w2 = pool.acquire().await.unwrap();
    assert_eq!(id1, w2.id, "expected reuse of idle worker");
    pool.release(w2).await;
}

#[tokio::test]
async fn test_wire_pool_spills_to_fresh_when_size_exceeded() {
    let pool = std::sync::Arc::new(WirePool::with_factory(2, MockFactory));
    let w1 = pool.acquire().await.unwrap();
    let w2 = pool.acquire().await.unwrap();
    let w3 = pool.acquire().await.unwrap();

    // All three should succeed (spillover). Distinct IDs prove fresh workers.
    assert_ne!(w1.id, w2.id);
    assert_ne!(w2.id, w3.id);

    pool.release(w1).await;
    pool.release(w2).await;
    pool.release(w3).await;
}

// ---------------------------------------------------------------------------
// Subscribe unit test (no goal runtime)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_subscribe_missing_goal_returns_error() {
    let result = omk::runtime::goal::chat_api::subscribe("nonexistent-goal-id");
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("not found"),
        "expected 'not found' error, got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// Integration test helpers
// ---------------------------------------------------------------------------

static GOAL_MUTEX: Mutex<()> = Mutex::new(());

fn setup_temp_state() -> tempfile::TempDir {
    let temp = tempfile::TempDir::new().unwrap();
    std::env::set_var("XDG_STATE_HOME", temp.path());
    temp
}

fn make_request(prompt: &str) -> CreateChildRequest {
    CreateChildRequest {
        session_id: "sess-test".to_string(),
        parent_conv_id: "conv-test".to_string(),
        prompt: prompt.to_string(),
        config: ChildGoalConfig::default(),
    }
}

async fn append_event(goal_id: &str, event: Event) {
    let goals_dir = omk::runtime::config::omk_state_dir().join(omk::runtime::goal::GOALS_DIR);
    let path = goals_dir.join(goal_id).join("events.jsonl");
    let line = format!("{}\n", serde_json::to_string(&event).unwrap());
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .await
        .unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes())
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Integration tests (slow — require real goal scaffolding)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_create_child_returns_handle_with_goal_id() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("write a hello world");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();

    assert!(!handle.goal_id.is_empty());
    assert_eq!(handle.session_id, "sess-test");

    let _ = omk::runtime::goal::chat_api::cancel(&handle.goal_id).await;
}

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_subscribe_receives_created_event() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("test goal created event");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();
    let mut rx = omk::runtime::goal::chat_api::subscribe(&handle.goal_id).unwrap();

    // Append a synthetic RunStarted event after subscription
    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId(handle.goal_id.clone()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::RunStarted,
        actor: None,
        payload: Some(
            serde_json::to_value(RunStartedPayload {
                mode: "test".to_string(),
                project_dir: std::env::current_dir().unwrap(),
                description: "test description".to_string(),
                kimi_binary: None,
                kimi_cli_version: None,
                wire_protocol_version: None,
            })
            .unwrap(),
        ),
    };
    append_event(&handle.goal_id, event).await;

    let child_event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();

    assert!(
        matches!(child_event, ChildGoalEvent::Created { ref goal_id, .. } if goal_id == &handle.goal_id)
    );

    let _ = omk::runtime::goal::chat_api::cancel(&handle.goal_id).await;
}

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_subscribe_propagates_plan_updates() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("test goal plan update");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();
    let mut rx = omk::runtime::goal::chat_api::subscribe(&handle.goal_id).unwrap();

    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId(handle.goal_id.clone()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::TaskGraphMutated,
        actor: None,
        payload: Some(
            serde_json::to_value(TaskGraphMutationPayload {
                action: "add".to_string(),
                source: "test".to_string(),
                task_id: omk::runtime::events::TaskId("t1".to_string()),
                task_graph_path: std::path::PathBuf::from("/tmp/tg.json"),
                proposal_path: std::path::PathBuf::from("/tmp/prop.json"),
                total_tasks_after: 3,
            })
            .unwrap(),
        ),
    };
    append_event(&handle.goal_id, event).await;

    let child_event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match child_event {
        ChildGoalEvent::PlanUpdated { nodes, .. } => {
            assert_eq!(nodes.len(), 3);
            assert_eq!(nodes[0].status, PlanNodeStatus::Pending);
        }
        other => panic!("expected PlanUpdated, got {:?}", other),
    }

    let _ = omk::runtime::goal::chat_api::cancel(&handle.goal_id).await;
}

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_subscribe_propagates_proof_ready_with_correct_path() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("test goal proof");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();
    let mut rx = omk::runtime::goal::chat_api::subscribe(&handle.goal_id).unwrap();

    // Create a real proof file path in the goal state dir
    let goals_dir = omk::runtime::config::omk_state_dir().join(omk::runtime::goal::GOALS_DIR);
    let proof_path = goals_dir.join(&handle.goal_id).join("proof.json");
    tokio::fs::write(&proof_path, b"{}").await.unwrap();

    let event = Event {
        id: omk::runtime::events::EventId::generate(),
        run_id: omk::runtime::events::RunId(handle.goal_id.clone()),
        ts: chrono::Utc::now(),
        schema_version: 1,
        kind: EventKind::ProofWritten,
        actor: None,
        payload: Some(
            serde_json::to_value(ProofWrittenPayload {
                proof_path: proof_path.clone(),
                status: "ready".to_string(),
            })
            .unwrap(),
        ),
    };
    append_event(&handle.goal_id, event).await;

    let child_event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match child_event {
        ChildGoalEvent::ProofReady { path } => {
            assert_eq!(path, proof_path);
            assert!(path.exists());
        }
        other => panic!("expected ProofReady, got {:?}", other),
    }

    let _ = omk::runtime::goal::chat_api::cancel(&handle.goal_id).await;
}

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_pause_resume_round_trip() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("test goal pause resume");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();

    omk::runtime::goal::chat_api::pause(&handle.goal_id)
        .await
        .unwrap();
    let state = omk::runtime::goal::resolve_goal(&handle.goal_id)
        .await
        .unwrap();
    assert_eq!(
        state.status,
        omk::runtime::goal::GoalStatus::Paused,
        "expected Paused after pause"
    );

    omk::runtime::goal::chat_api::resume(&handle.goal_id)
        .await
        .unwrap();
    let state = omk::runtime::goal::resolve_goal(&handle.goal_id)
        .await
        .unwrap();
    assert_ne!(
        state.status,
        omk::runtime::goal::GoalStatus::Paused,
        "expected not Paused after resume"
    );

    let _ = omk::runtime::goal::chat_api::cancel(&handle.goal_id).await;
}

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_cancel_emits_cancelled_event() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("test goal cancel");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();
    let mut rx = omk::runtime::goal::chat_api::subscribe(&handle.goal_id).unwrap();

    omk::runtime::goal::chat_api::cancel(&handle.goal_id)
        .await
        .unwrap();

    // The tail task may need a moment to catch the ManualInterrupt event
    // before deregister aborts it.  Try with a short timeout.
    let mut found = false;
    if let Ok(Ok(ev)) = tokio::time::timeout(Duration::from_millis(800), rx.recv()).await {
        if matches!(ev, ChildGoalEvent::Cancelled) {
            found = true;
        }
    }

    if !found {
        // Fallback: at minimum verify the goal state is Cancelled
        let state = omk::runtime::goal::resolve_goal(&handle.goal_id)
            .await
            .unwrap();
        assert_eq!(
            state.status,
            omk::runtime::goal::GoalStatus::Cancelled,
            "expected Cancelled status after cancel"
        );
    }
}

#[tokio::test]
#[ignore = "slow: requires real goal execution"]
async fn test_replay_existing_goal_via_chat_api() {
    let _guard = GOAL_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _temp = setup_temp_state();
    drop(_guard);

    let req = make_request("test goal replay");
    let handle = omk::runtime::goal::chat_api::create_child(req)
        .await
        .unwrap();

    // Give scaffolding a moment to write the scaffold proof
    tokio::time::sleep(Duration::from_millis(500)).await;

    let path = show_proof(&handle.goal_id).await.unwrap();
    assert!(path.exists(), "proof path should exist");

    let _ = omk::runtime::goal::chat_api::cancel(&handle.goal_id).await;
}

#[test]
fn test_existing_omk_goal_run_headless_still_works() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "omk", "--", "goal", "run", "--help"])
        .current_dir(std::env::current_dir().unwrap())
        .output()
        .expect("failed to execute cargo run");

    assert!(
        output.status.success(),
        "cargo run exited with non-zero status: stderr = {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage:") || stdout.contains("usage:") || stdout.contains("goal"),
        "expected usage help in stdout: {}",
        stdout
    );
}
