//! Edge-case coverage for `EventReader`.
//!
//! These tests pin current behavior for malformed JSONL, blank/whitespace
//! lines, unknown event kinds, stable ordering, and the public read helpers
//! that previously lacked direct coverage.

use std::path::PathBuf;

use tempfile::TempDir;

use crate::runtime::events::kind::EVENT_SCHEMA_VERSION;
use crate::runtime::events::reader::payload_string;
use crate::runtime::events::{Event, EventKind, EventReader, EventWriter, RunId};

/// Allocate a temp dir and return a path for `events.jsonl` inside it.
/// The returned `TempDir` must be bound (commonly as `_tmp`) so the
/// directory survives until the test scope ends.
fn temp_jsonl() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("events.jsonl");
    (tmp, path)
}

#[tokio::test]
async fn reader_returns_empty_for_missing_file() {
    let (_tmp, path) = temp_jsonl();
    assert!(EventReader::read_all(&path).await.unwrap().is_empty());
    let s = EventReader::summary(&path).await.unwrap();
    assert_eq!(
        (
            s.total_lines,
            s.valid_events,
            s.parse_failures,
            s.empty_lines
        ),
        (0, 0, 0, 0)
    );
}

#[tokio::test]
async fn reader_handles_crlf_line_endings() {
    let (_tmp, path) = temp_jsonl();
    let e1 = Event::new(RunId("r".into()), EventKind::RunStarted);
    let e2 = Event::new(RunId("r".into()), EventKind::RunCompleted);
    let body = format!(
        "{}\r\n{}\r\n",
        serde_json::to_string(&e1).unwrap(),
        serde_json::to_string(&e2).unwrap()
    );
    tokio::fs::write(&path, body).await.unwrap();

    let events = EventReader::read_all(&path).await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].kind, EventKind::RunStarted));
    assert!(matches!(events[1].kind, EventKind::RunCompleted));
}

#[tokio::test]
async fn reader_handles_unterminated_final_line() {
    let (_tmp, path) = temp_jsonl();
    let e = Event::new(RunId("r".into()), EventKind::RunStarted);
    tokio::fs::write(&path, serde_json::to_string(&e).unwrap())
        .await
        .unwrap();
    assert_eq!(EventReader::read_all(&path).await.unwrap().len(), 1);
}

#[tokio::test]
async fn reader_treats_whitespace_only_lines_as_blank() {
    let (_tmp, path) = temp_jsonl();
    let e = Event::new(RunId("r".into()), EventKind::RunStarted);
    let json = serde_json::to_string(&e).unwrap();
    tokio::fs::write(&path, format!("{json}\n   \n\t\t\n{json}\n"))
        .await
        .unwrap();

    assert_eq!(EventReader::read_all(&path).await.unwrap().len(), 2);
    let s = EventReader::summary(&path).await.unwrap();
    assert_eq!((s.empty_lines, s.valid_events, s.parse_failures), (2, 2, 0));
}

#[tokio::test]
async fn reader_skips_non_object_json_lines() {
    let (_tmp, path) = temp_jsonl();
    let valid = Event::new(RunId("r".into()), EventKind::RunStarted);
    let valid_json = serde_json::to_string(&valid).unwrap();
    let body = format!("[]\n\"plain\"\n42\nnull\n{valid_json}\n[1,2,3]\ntrue\n");
    tokio::fs::write(&path, body).await.unwrap();

    let events = EventReader::read_all(&path).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, EventKind::RunStarted));
}

#[tokio::test]
async fn reader_skips_unknown_event_kind() {
    // Pin current behavior: a future EventKind variant present in the log is
    // skipped (counted as parse failure), not coerced into a known variant.
    // If the schema gains a fallback kind, update this test alongside it.
    let (_tmp, path) = temp_jsonl();
    // Twin envelopes — identical shape, only `kind` differs. This proves the
    // discriminant is the kind field and not some other missing/malformed
    // part of the hand-crafted envelope.
    let envelope = |id: &str, kind: &str| {
        serde_json::json!({
            "id": id,
            "run_id": "r",
            "ts": "2026-01-01T00:00:00Z",
            "schema_version": EVENT_SCHEMA_VERSION,
            "kind": kind,
            "actor": null,
        })
    };
    let known = envelope("known-id", "run_started");
    let unknown = envelope("future-id", "future_unknown_kind");
    tokio::fs::write(&path, format!("{known}\n{unknown}\n"))
        .await
        .unwrap();

    let events = EventReader::read_all(&path).await.unwrap();
    assert_eq!(events.len(), 1, "only the known-kind envelope must parse");
    assert!(matches!(events[0].kind, EventKind::RunStarted));
    let s = EventReader::summary(&path).await.unwrap();
    assert_eq!((s.total_lines, s.valid_events, s.parse_failures), (2, 1, 1));
}

#[tokio::test]
async fn reader_preserves_insertion_order_with_blanks_and_malformed() {
    let (_tmp, path) = temp_jsonl();
    let run_id = RunId("r".into());
    let e1 = Event::new(run_id.clone(), EventKind::RunStarted);
    let e2 = Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor("w-a");
    let e3 = Event::new(run_id.clone(), EventKind::TaskStarted);
    let e4 = Event::new(run_id, EventKind::RunCompleted);

    let body = format!(
        "{}\n\n{}\nnot json\n{}\n   \n{}\n",
        serde_json::to_string(&e1).unwrap(),
        serde_json::to_string(&e2).unwrap(),
        serde_json::to_string(&e3).unwrap(),
        serde_json::to_string(&e4).unwrap(),
    );
    tokio::fs::write(&path, body).await.unwrap();

    let events = EventReader::read_all(&path).await.unwrap();
    let ids: Vec<String> = events.into_iter().map(|e| e.id.0).collect();
    assert_eq!(ids, vec![e1.id.0, e2.id.0, e3.id.0, e4.id.0]);
}

#[tokio::test]
async fn reader_filtered_preserves_insertion_order() {
    let (_tmp, path) = temp_jsonl();
    let run_id = RunId("r".into());
    let started = Event::new(run_id.clone(), EventKind::RunStarted);
    let task_a = Event::new(run_id.clone(), EventKind::TaskStarted).with_actor("a");
    let task_b = Event::new(run_id.clone(), EventKind::TaskStarted).with_actor("b");
    let task_c = Event::new(run_id.clone(), EventKind::TaskStarted).with_actor("c");
    let completed = Event::new(run_id, EventKind::RunCompleted);

    EventWriter::new(&path)
        .append_many(&[
            started,
            task_a.clone(),
            task_b.clone(),
            task_c.clone(),
            completed,
        ])
        .await
        .unwrap();

    let task_events = EventReader::read_filtered(&path, &[EventKind::TaskStarted])
        .await
        .unwrap();
    let ids: Vec<String> = task_events.into_iter().map(|e| e.id.0).collect();
    assert_eq!(ids, vec![task_a.id.0, task_b.id.0, task_c.id.0]);
}

#[tokio::test]
async fn reader_filters_by_worker_actor() {
    let (_tmp, path) = temp_jsonl();
    let run_id = RunId("r".into());

    EventWriter::new(&path)
        .append_many(&[
            Event::new(run_id.clone(), EventKind::TaskStarted).with_actor("alpha"),
            Event::new(run_id.clone(), EventKind::TaskStarted).with_actor("beta"),
            Event::new(run_id.clone(), EventKind::TaskStarted).with_actor("alpha"),
            Event::new(run_id, EventKind::RunCompleted),
        ])
        .await
        .unwrap();

    let alpha = EventReader::read_for_worker(&path, "alpha").await.unwrap();
    assert_eq!(alpha.len(), 2);
    assert!(alpha.iter().all(|e| e.actor.as_deref() == Some("alpha")));
    assert!(EventReader::read_for_worker(&path, "gamma")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn reader_range_inclusive_at_endpoints() {
    let (_tmp, path) = temp_jsonl();
    let base = chrono::Utc::now();
    let make = |kind: EventKind, offset_secs: i64| {
        let mut e = Event::new(RunId("r".into()), kind);
        e.ts = base + chrono::Duration::seconds(offset_secs);
        e
    };

    let events = [
        make(EventKind::RunStarted, -10),
        make(EventKind::WorkerStarted, 0),
        make(EventKind::TaskStarted, 5),
        make(EventKind::TaskCompleted, 10),
        make(EventKind::RunCompleted, 20),
    ];
    EventWriter::new(&path).append_many(&events).await.unwrap();

    let in_range = EventReader::read_range(&path, events[1].ts, events[3].ts)
        .await
        .unwrap();
    assert_eq!(in_range.len(), 3, "range endpoints must be inclusive");
    assert!(matches!(in_range[0].kind, EventKind::WorkerStarted));
    assert!(matches!(in_range[2].kind, EventKind::TaskCompleted));
}

#[test]
fn payload_string_handles_missing_payload_or_key() {
    let no_payload = Event::new(RunId("r".into()), EventKind::RunStarted);
    assert!(payload_string(&no_payload, "anything").is_none());

    let with_payload = Event::new(RunId("r".into()), EventKind::TaskStarted)
        .with_payload(serde_json::json!({ "task_id": "t-1", "count": 7 }))
        .unwrap();
    assert_eq!(
        payload_string(&with_payload, "task_id").as_deref(),
        Some("t-1")
    );
    assert!(payload_string(&with_payload, "missing").is_none());
    assert!(
        payload_string(&with_payload, "count").is_none(),
        "non-string payload values must yield None"
    );
}

#[test]
fn payload_string_handles_zero_key_indirection() {
    // `payload_string` falls back to value.get("0") when the value is not
    // itself a string. Pin this branch — used for wrapped identifier objects —
    // and verify a non-string inner value still yields None.
    let wrapped = Event::new(RunId("r".into()), EventKind::TaskStarted)
        .with_payload(serde_json::json!({ "task_id": { "0": "wrapped-id" } }))
        .unwrap();
    assert_eq!(
        payload_string(&wrapped, "task_id").as_deref(),
        Some("wrapped-id")
    );

    let non_string_inner = Event::new(RunId("r".into()), EventKind::TaskStarted)
        .with_payload(serde_json::json!({ "task_id": { "0": 42 } }))
        .unwrap();
    assert!(payload_string(&non_string_inner, "task_id").is_none());
}
