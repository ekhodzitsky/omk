use std::path::{Path, PathBuf};
use std::sync::Arc;

use omk::runtime::conversation::escalation_log::{
    entry_from_event, EscalationKind, EscalationLogEntry, EscalationLogWriter,
};
use omk::runtime::conversation::{BusEvent, EventBus, Intent};
use tempfile::TempDir;

fn temp_state_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

fn parse_lines(path: &Path) -> Vec<EscalationLogEntry> {
    let content = std::fs::read_to_string(path).unwrap();
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn test_writer_creates_file_on_spawn() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir.clone(), rx).unwrap();
    let log_path = writer.log_path().to_path_buf();

    // Publish one event so the file is actually created.
    bus.publish(BusEvent::WorkerStarted {
        worker_id: "w1".into(),
        kind: "edit".into(),
        task: "rename".into(),
    });

    // Give the actor a moment to open and write.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    writer.shutdown().await.unwrap();
    assert!(log_path.exists(), "log file should exist after first write");
}

#[tokio::test]
async fn test_router_escalation_large_writes_entry() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    bus.publish(BusEvent::RouterEscalating {
        intent: Intent::Large,
        target_mode: omk::runtime::conversation::ActiveMode::GoalRun,
        preflight: true,
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    let entries = parse_lines(&log_path);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, EscalationKind::RouterEscalation);
}

#[tokio::test]
async fn test_router_escalation_trivial_does_not_write() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    bus.publish(BusEvent::RouterEscalating {
        intent: Intent::Trivial,
        target_mode: omk::runtime::conversation::ActiveMode::DirectLlm,
        preflight: false,
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    if log_path.exists() {
        let entries = parse_lines(&log_path);
        assert!(
            entries.is_empty(),
            "trivial escalation should not be logged"
        );
    }
}

#[tokio::test]
async fn test_worker_lifecycle_writes_started_and_completed() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    bus.publish(BusEvent::WorkerStarted {
        worker_id: "w1".into(),
        kind: "edit".into(),
        task: "rename".into(),
    });
    bus.publish(BusEvent::WorkerCompleted {
        worker_id: "w1".into(),
        files_touched: 3,
        ok: true,
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    let entries = parse_lines(&log_path);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].kind, EscalationKind::WorkerStarted);
    assert_eq!(entries[1].kind, EscalationKind::WorkerCompleted);
    assert!(entries[1].summary.contains("completed"));
}

#[tokio::test]
async fn test_worker_failure_writes_failed_kind() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    bus.publish(BusEvent::WorkerStarted {
        worker_id: "w1".into(),
        kind: "edit".into(),
        task: "rename".into(),
    });
    bus.publish(BusEvent::WorkerCompleted {
        worker_id: "w1".into(),
        files_touched: 0,
        ok: false,
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    let entries = parse_lines(&log_path);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].kind, EscalationKind::WorkerCompleted);
    assert!(entries[1].summary.contains("failed"));
}

#[tokio::test]
async fn test_multiple_events_appended_in_order() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    for i in 0..5 {
        bus.publish(BusEvent::WorkerStarted {
            worker_id: format!("w{i}"),
            kind: "test".into(),
            task: format!("task {i}"),
        });
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    let entries = parse_lines(&log_path);
    assert_eq!(entries.len(), 5);
    for (i, entry) in entries.iter().enumerate() {
        assert!(entry.summary.contains(&format!("task {i}")));
    }
}

#[tokio::test]
async fn test_concurrent_writes_are_line_atomic() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = Arc::new(EventBus::new());
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    let mut handles = Vec::new();
    for i in 0..10 {
        let bus = Arc::clone(&bus);
        handles.push(tokio::spawn(async move {
            bus.publish(BusEvent::WorkerStarted {
                worker_id: format!("w{i}"),
                kind: "test".into(),
                task: format!("task {i}"),
            });
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    let entries = parse_lines(&log_path);
    assert_eq!(entries.len(), 10, "expected 10 distinct lines");

    // Every line must parse as valid JSON (already ensured by parse_lines).
    // Additional sanity: no merged / interleaved JSON.
    for entry in &entries {
        assert!(entry.summary.starts_with("test: task "));
    }
}

#[tokio::test]
async fn test_writer_shutdown_flushes_pending() {
    let (_dir, state_dir) = temp_state_dir();
    let bus = EventBus::new();
    let rx = bus.subscribe();

    let writer = EscalationLogWriter::spawn(state_dir, rx).unwrap();

    bus.publish(BusEvent::WorkerStarted {
        worker_id: "w1".into(),
        kind: "edit".into(),
        task: "rename".into(),
    });

    // No sleep — shutdown should flush.
    let log_path = writer.log_path().to_path_buf();
    writer.shutdown().await.unwrap();

    let entries = parse_lines(&log_path);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, EscalationKind::WorkerStarted);
}

#[test]
fn test_entry_from_event_helper_filters_correctly() {
    // Trivial router escalation -> None
    assert!(entry_from_event(&BusEvent::RouterEscalating {
        intent: Intent::Trivial,
        target_mode: omk::runtime::conversation::ActiveMode::DirectLlm,
        preflight: false,
    })
    .is_none());

    // Small router escalation -> Some
    assert!(entry_from_event(&BusEvent::RouterEscalating {
        intent: Intent::Small,
        target_mode: omk::runtime::conversation::ActiveMode::WireWorker,
        preflight: false,
    })
    .is_some());

    // WorkerStarted -> Some
    assert!(entry_from_event(&BusEvent::WorkerStarted {
        worker_id: "w1".into(),
        kind: "edit".into(),
        task: "rename".into(),
    })
    .is_some());

    // WorkerCompleted -> Some
    assert!(entry_from_event(&BusEvent::WorkerCompleted {
        worker_id: "w1".into(),
        files_touched: 0,
        ok: true,
    })
    .is_some());

    // CostDelta -> None
    assert!(entry_from_event(&BusEvent::CostDelta {
        source: "llm".into(),
        tokens_in: 100,
        tokens_out: 50,
        usd: 0.01,
    })
    .is_none());

    // Refused -> Some
    assert!(entry_from_event(&BusEvent::Refused {
        reason: "cap".into(),
        intent: Intent::Large,
    })
    .is_some());
}
