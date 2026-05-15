use tempfile::TempDir;

use crate::runtime::config::EVENTS_FILE;
use crate::runtime::events::{EventKind, EventReader};
use tokio_util::sync::CancellationToken;

use super::*;

#[tokio::test]
async fn test_run_with_cancel_reason_records_custom_task_failure_error() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Pause this work");

    let worker = make_spec(&tmp, "worker-cancelled").await;
    let cancel = CancellationToken::new();
    cancel.cancel();

    let summary = runner
        .run_with_cancel_reason(std::slice::from_ref(&worker), &cancel, "cancelled by user")
        .await
        .unwrap();

    assert_eq!(summary.completed, 0);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.cancelled, 1);
    assert_eq!(summary.total, 1);

    let events = EventReader::read_all(&tmp.path().join("state").join(EVENTS_FILE))
        .await
        .unwrap();
    assert!(events.iter().any(|event| {
        event.kind == EventKind::TaskFailed
            && event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("task_id"))
                .and_then(|value| value.as_str())
                == Some("task-1")
            && event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("error"))
                .and_then(|value| value.as_str())
                == Some("cancelled by user")
    }));
}
