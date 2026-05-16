use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};

use crate::runtime::events::{Event, EventReader};

#[derive(Debug)]
pub struct EventStream {
    path: PathBuf,
    last_position: u64,
}

impl EventStream {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            last_position: 0,
        }
    }

    /// Read new events since last poll (non-blocking)
    pub async fn poll(&mut self) -> Result<Vec<Event>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&self.path)
            .await?;
        let mut reader = BufReader::new(file);
        let metadata = reader.get_ref().metadata().await?;
        let file_len = metadata.len();

        if file_len < self.last_position {
            // File was truncated or replaced; start from beginning
            self.last_position = 0;
        }

        reader
            .seek(tokio::io::SeekFrom::Start(self.last_position))
            .await?;

        let mut events = Vec::new();
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<Event>(trimmed) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::warn!(error = %e, line = trimmed, "Skipping malformed event line");
                }
            }
        }

        self.last_position = reader.stream_position().await?;
        Ok(events)
    }

    /// Read all events from the beginning
    pub async fn read_all(&self) -> Result<Vec<Event>> {
        EventReader::read_all(&self.path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::events::{Event, EventKind, EventWriter, RunId};

    #[tokio::test]
    async fn event_stream_poll_reads_incremental_events() {
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

        // Write second event
        let e2 = Event::new(RunId("run-1".to_string()), EventKind::WorkerStarted).with_actor("w1");
        writer.append(&e2).await.unwrap();

        let events = stream.poll().await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].kind, EventKind::WorkerStarted));

        // No new events
        let events = stream.poll().await.unwrap();
        assert!(events.is_empty());
    }
}
