use std::path::Path;
use std::path::PathBuf;

use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

use crate::runtime::events::kind::RunStartedPayload;
use crate::runtime::events::kind::EVENT_SCHEMA_VERSION;
use crate::runtime::events::reader::payload_string;
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventReader, EventWriter, GateId, RunId, TaskId, WorkerId,
};

#[test]
fn event_roundtrip() {
    let event = Event::new(RunId("test".to_string()), EventKind::RunStarted)
        .with_actor("worker-a")
        .with_payload(RunStartedPayload {
            mode: "team".to_string(),
            project_dir: PathBuf::from("/tmp/test"),
            description: "test run".to_string(),
            kimi_binary: None,
            kimi_cli_version: None,
            wire_protocol_version: None,
        })
        .unwrap();

    let json = serde_json::to_string(&event).unwrap();
    let restored: Event = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.run_id.0, "test");
    assert_eq!(restored.actor, Some("worker-a".to_string()));
    assert_eq!(restored.schema_version, EVENT_SCHEMA_VERSION);
}

#[tokio::test]
async fn writer_reader_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");

    let writer = EventWriter::new(&path);
    let run_id = RunId("run-1".to_string());

    let e1 = Event::new(run_id.clone(), EventKind::RunStarted);
    let e2 = Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor("w1");
    let e3 = Event::new(run_id.clone(), EventKind::RunCompleted);

    writer.append(&e1).await.unwrap();
    writer.append(&e2).await.unwrap();
    writer.append(&e3).await.unwrap();

    let events = EventReader::read_all(&path).await.unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0].kind, EventKind::RunStarted));
    assert!(matches!(events[2].kind, EventKind::RunCompleted));
}

#[tokio::test]
async fn writer_concurrent_appends_preserve_jsonl_boundaries() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");
    let writer = EventWriter::new(&path);
    let run_id = RunId("run-concurrent".to_string());

    let mut handles = Vec::new();
    for idx in 0..32 {
        let writer = writer.clone();
        let run_id = run_id.clone();
        handles.push(tokio::spawn(async move {
            let event = Event::new(run_id, EventKind::TaskOutput)
                .with_payload(serde_json::json!({ "idx": idx }))
                .unwrap();
            writer.append(&event).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let summary = EventReader::summary(&path).await.unwrap();
    assert_eq!(summary.valid_events, 32);
    assert_eq!(summary.parse_failures, 0);
}

#[tokio::test]
async fn reader_tolerates_partial_trailing_line() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");

    let valid = Event::new(RunId("r".to_string()), EventKind::RunStarted);
    let valid_json = serde_json::to_string(&valid).unwrap();
    let mut file = tokio::fs::File::create(&path).await.unwrap();
    file.write_all(format!("{}\n", valid_json).as_bytes())
        .await
        .unwrap();
    file.write_all(b"{\"partial\": true").await.unwrap(); // incomplete JSON

    let events = EventReader::read_all(&path).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, EventKind::RunStarted));
}

#[tokio::test]
async fn reader_tolerates_malformed_lines() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");

    let valid = Event::new(RunId("r".to_string()), EventKind::RunStarted);
    let valid_json = serde_json::to_string(&valid).unwrap();
    let mut file = tokio::fs::File::create(&path).await.unwrap();
    file.write_all(format!("{}\n", valid_json).as_bytes())
        .await
        .unwrap();
    file.write_all(b"not json at all\n").await.unwrap();
    file.write_all(b"{}\n").await.unwrap(); // empty object - will fail because it lacks required fields

    let events = EventReader::read_all(&path).await.unwrap();
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn reader_summary() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");

    let valid = Event::new(RunId("r".to_string()), EventKind::RunStarted);
    let valid_json = serde_json::to_string(&valid).unwrap();
    let mut file = tokio::fs::File::create(&path).await.unwrap();
    file.write_all(format!("{}\n", valid_json).as_bytes())
        .await
        .unwrap();
    file.write_all(b"bad\n").await.unwrap();
    file.write_all(b"\n").await.unwrap();

    let summary = EventReader::summary(&path).await.unwrap();
    assert_eq!(summary.total_lines, 3);
    assert_eq!(summary.valid_events, 1);
    assert_eq!(summary.parse_failures, 1);
    assert_eq!(summary.empty_lines, 1);
}

#[tokio::test]
async fn reader_filter_by_kind() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");

    let writer = EventWriter::new(&path);
    let run_id = RunId("run-1".to_string());

    writer
        .append_many(&[
            Event::new(run_id.clone(), EventKind::RunStarted),
            Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor("w1"),
            Event::new(run_id.clone(), EventKind::RunCompleted),
        ])
        .await
        .unwrap();

    let filtered =
        EventReader::read_filtered(&path, &[EventKind::RunStarted, EventKind::RunCompleted])
            .await
            .unwrap();
    assert_eq!(filtered.len(), 2);
}

#[tokio::test]
async fn reader_filters_by_task_and_gate() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");

    let writer = EventWriter::new(&path);
    let run_id = RunId("run-1".to_string());
    let builder = EventBuilder::new(run_id);

    writer
        .append_many(&[
            builder
                .task_claimed(
                    TaskId("task-1".to_string()),
                    WorkerId("worker-1".to_string()),
                    60,
                )
                .unwrap(),
            builder
                .task_completed(
                    TaskId("task-1".to_string()),
                    WorkerId("worker-1".to_string()),
                    Some("done"),
                )
                .unwrap(),
            builder.gate_passed_by_name("fmt").unwrap(),
            builder.gate_failed_by_name("test").unwrap(),
        ])
        .await
        .unwrap();

    let task_events = EventReader::read_for_task(&path, "task-1").await.unwrap();
    assert_eq!(task_events.len(), 2);
    assert!(task_events
        .iter()
        .all(|e| payload_string(e, "task_id").as_deref() == Some("task-1")));

    let gate_events = EventReader::read_for_gate(&path, "fmt").await.unwrap();
    assert_eq!(gate_events.len(), 1);
    assert_eq!(
        payload_string(&gate_events[0], "gate_id").as_deref(),
        Some("fmt")
    );

    let named_gate_events = EventReader::read_for_gate(&path, "test").await.unwrap();
    assert_eq!(named_gate_events.len(), 1);
    assert!(matches!(named_gate_events[0].kind, EventKind::GateFailed));
}

#[test]
fn event_builder_helpers() {
    let run_id = RunId::generate();
    let builder = EventBuilder::new(run_id.clone());

    let e1 = builder
        .run_started("team", Path::new("/tmp"), "test")
        .unwrap();
    assert!(matches!(e1.kind, EventKind::RunStarted));

    let e2 = builder
        .worker_started(WorkerId("w1".to_string()), "coder")
        .unwrap();
    assert!(matches!(e2.kind, EventKind::WorkerStarted));
    assert_eq!(e2.actor, Some("w1".to_string()));

    let e3 = builder.run_completed();
    assert!(matches!(e3.kind, EventKind::RunCompleted));
}

#[test]
fn run_started_can_include_kimi_metadata() {
    let run_id = RunId::generate();
    let event = EventBuilder::new(run_id)
        .run_started_with_kimi_metadata(
            "team",
            Path::new("/tmp"),
            "test",
            Some("/usr/local/bin/kimi".to_string()),
            Some("kimi version 1.41.0".to_string()),
            Some("1.9".to_string()),
        )
        .unwrap();

    let payload: RunStartedPayload = serde_json::from_value(event.payload.unwrap()).unwrap();
    assert_eq!(payload.kimi_binary.as_deref(), Some("/usr/local/bin/kimi"));
    assert_eq!(
        payload.kimi_cli_version.as_deref(),
        Some("kimi version 1.41.0")
    );
    assert_eq!(payload.wire_protocol_version.as_deref(), Some("1.9"));
}

#[test]
fn command_and_gate_events_can_include_evidence_payload() {
    let run_id = RunId::generate();
    let builder = EventBuilder::new(run_id);

    let started = builder
        .command_started(GateId("fmt".to_string()), "fmt", "cargo fmt --check", 120)
        .unwrap();
    assert!(matches!(started.kind, EventKind::CommandStarted));
    let started_payload = started.payload.unwrap();
    assert_eq!(
        started_payload.get("command_line").and_then(|v| v.as_str()),
        Some("cargo fmt --check")
    );
    assert_eq!(
        started_payload.get("timeout_secs").and_then(|v| v.as_u64()),
        Some(120)
    );

    let finished = builder
        .command_finished(
            GateId("fmt".to_string()),
            "fmt",
            "cargo fmt --check",
            Some(0),
            false,
            Some("ok"),
            Some(""),
            Some("/tmp/gates/fmt.log"),
        )
        .unwrap();
    assert!(matches!(finished.kind, EventKind::CommandFinished));
    let finished_payload = finished.payload.unwrap();
    assert_eq!(
        finished_payload
            .get("command_line")
            .and_then(|v| v.as_str()),
        Some("cargo fmt --check")
    );
    assert_eq!(
        finished_payload.get("exit_code").and_then(|v| v.as_i64()),
        Some(0)
    );
    assert_eq!(
        finished_payload.get("timed_out").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        finished_payload.get("output_path").and_then(|v| v.as_str()),
        Some("/tmp/gates/fmt.log")
    );

    let gate_passed = builder
        .gate_passed_with_evidence(
            GateId("fmt".to_string()),
            "fmt",
            true,
            Some("cargo fmt --check"),
            Some(0),
            false,
            Some("ok"),
            Some(""),
            Some("/tmp/gates/fmt.log"),
            Some(120),
        )
        .unwrap();
    assert!(matches!(gate_passed.kind, EventKind::GatePassed));
    let gate_payload = gate_passed.payload.unwrap();
    assert_eq!(
        gate_payload.get("stdout_summary").and_then(|v| v.as_str()),
        Some("ok")
    );
    assert_eq!(
        gate_payload.get("timeout_secs").and_then(|v| v.as_u64()),
        Some(120)
    );
}

#[test]
fn event_serde_roundtrip_across_kinds_and_actor_shapes() {
    let kinds = [
        EventKind::RunStarted,
        EventKind::RunCompleted,
        EventKind::RunFailed,
        EventKind::WorkerStarted,
        EventKind::WorkerHeartbeat,
        EventKind::WorkerStalled,
        EventKind::WorkerDead,
        EventKind::WorkerRecovered,
        EventKind::TaskProposed,
        EventKind::TaskAccepted,
        EventKind::TaskRejected,
        EventKind::TaskGraphMutated,
        EventKind::TaskClaimed,
        EventKind::TaskStarted,
        EventKind::TaskOutput,
        EventKind::TaskCompleted,
        EventKind::TaskFailed,
        EventKind::FileChanged,
        EventKind::CommandStarted,
        EventKind::CommandFinished,
        EventKind::GatePassed,
        EventKind::GateFailed,
        EventKind::RetryScheduled,
        EventKind::ProofWritten,
        EventKind::ManualInterrupt,
        EventKind::GoalPaused,
        EventKind::GoalResumed,
        EventKind::GoalBudgetExhausted,
        EventKind::GoalBudgetExtended,
        EventKind::BudgetCheckpoint,
    ];
    let cases = [
        ("run-a", None),
        ("run_123", Some("worker-1")),
        ("RUN-mixed_42", Some("scheduler")),
    ];

    for kind in kinds {
        for (run_id, actor) in cases {
            let mut event = Event::new(RunId(run_id.to_string()), kind.clone());
            if let Some(actor) = actor {
                event = event.with_actor(actor);
            }

            let json = serde_json::to_string(&event).unwrap();
            let restored: Event = serde_json::from_str(&json).unwrap();

            assert_eq!(restored.run_id, event.run_id);
            assert_eq!(restored.actor, event.actor);
            assert_eq!(restored.kind, event.kind);
            assert_eq!(restored.schema_version, event.schema_version);
        }
    }
}
