use anyhow::Result;

use super::Event;

/// Trait for event sinks — consumers that receive [`Event`] records.
///
/// Implementations may write to files, send over channels, or buffer
/// in-memory for testing.
#[allow(async_fn_in_trait)]
pub trait EventSink: Send + Sync {
    /// Send a single event to the sink.
    async fn send_event(&self, event: &Event) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::events::{EventKind, RunId};
    use crate::test_helpers::MockEventSink;

    #[tokio::test]
    async fn mock_event_sink_records_events() {
        let sink = MockEventSink::new();
        let event = Event::new(RunId("run-1".to_string()), EventKind::RunStarted);
        sink.send_event(&event).await.unwrap();

        let records = sink.take_records().await;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].run_id.0, "run-1");
        assert!(matches!(records[0].kind, EventKind::RunStarted));
    }

    #[tokio::test]
    async fn mock_event_sink_take_clears_buffer() {
        let sink = MockEventSink::new();
        let event = Event::new(RunId("run-2".to_string()), EventKind::WorkerStarted);
        sink.send_event(&event).await.unwrap();

        let first = sink.take_records().await;
        assert_eq!(first.len(), 1);

        let second = sink.take_records().await;
        assert!(second.is_empty());
    }

    #[tokio::test]
    async fn mock_event_sink_peek_does_not_clear() {
        let sink = MockEventSink::new();
        let event = Event::new(RunId("run-3".to_string()), EventKind::TaskCompleted);
        sink.send_event(&event).await.unwrap();

        let peek1 = sink.records().await;
        let peek2 = sink.records().await;
        assert_eq!(peek1.len(), 1);
        assert_eq!(peek2.len(), 1);
    }
}
