use anyhow::Result;
use std::path::PathBuf;
use tracing::debug;

use crate::runtime::events::Event;

/// Append-only JSONL event writer.
#[derive(Clone)]
pub struct EventWriter {
    path: PathBuf,
}

impl EventWriter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Append a single event atomically-ish: open, write, flush, close.
    /// This is not OS-level atomic, but it minimizes the window for corruption.
    pub async fn append(&self, event: &Event) -> Result<()> {
        let mut line = serde_json::to_vec(event)?;
        line.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&line).await?;
        file.flush().await?;
        debug!(event_id = %event.id, "Appended event");
        Ok(())
    }

    pub async fn append_many(&self, events: &[Event]) -> Result<()> {
        let mut buffer = Vec::new();
        for event in events {
            serde_json::to_writer(&mut buffer, event)?;
            buffer.push(b'\n');
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&buffer).await?;
        file.flush().await?;
        Ok(())
    }
}
