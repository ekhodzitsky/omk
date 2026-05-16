use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::runtime::events::Event;

/// Channel capacity for the writer actor.
///
/// Bounded so a runaway producer cannot OOM the process; large enough that
/// typical event bursts (gate-pass spam, task-claim cascades) do not stall
/// senders waiting on the actor. Backpressure surfaces as a slow `.send()`
/// at the producer, which is the desired signal.
const WRITER_CHANNEL_CAPACITY: usize = 1024;

/// A single writer task that owns one append-only JSONL file.
///
/// All clones of a [`JsonlWriter`] (and the [`EventWriter`] wrapping it)
/// funnel through the same mpsc channel, so concurrent producers never
/// interleave partial line writes on the underlying file — even when the
/// host filesystem does not guarantee O_APPEND atomicity for buffered
/// writes larger than PIPE_BUF.
///
/// The actor task lives until every cloned sender is dropped; that drains
/// remaining queued messages, flushes the file, and exits naturally.
#[derive(Clone, Debug)]
pub struct JsonlWriter {
    tx: mpsc::Sender<WriterMsg>,
}

struct WriterMsg {
    payload: Vec<u8>,
    ack: oneshot::Sender<Result<()>>,
}

impl JsonlWriter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (tx, rx) = mpsc::channel::<WriterMsg>(WRITER_CHANNEL_CAPACITY);
        tokio::spawn(writer_task(path, rx));
        Self { tx }
    }

    /// Append a JSONL fragment to the file.
    ///
    /// `payload` should already include any trailing newline(s) needed to
    /// terminate the line(s) — this method does not add one.
    pub async fn append_line(&self, payload: Vec<u8>) -> Result<()> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.tx
            .send(WriterMsg {
                payload,
                ack: ack_tx,
            })
            .await
            .map_err(|_| anyhow!("JsonlWriter actor has shut down before send"))?;
        ack_rx
            .await
            .map_err(|_| anyhow!("JsonlWriter actor dropped ack channel"))?
    }
}

async fn writer_task(path: PathBuf, mut rx: mpsc::Receiver<WriterMsg>) {
    // Open once; reuse the handle across the actor's lifetime. The previous
    // open-write-close-per-call pattern not only re-opened the inode on each
    // event (high syscall overhead) but also widened the race window on
    // filesystems without strong O_APPEND guarantees.
    let mut file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
    {
        Ok(f) => f,
        Err(e) => {
            warn!(error = %e, path = %path.display(), "JsonlWriter failed to open file; failing all incoming writes");
            // Drain so senders unblock with a clear error rather than
            // hanging on a never-completing channel send.
            while let Some(msg) = rx.recv().await {
                let _ = msg.ack.send(Err(anyhow!(
                    "JsonlWriter could not open '{}': {}",
                    path.display(),
                    e
                )));
            }
            return;
        }
    };

    while let Some(msg) = rx.recv().await {
        let result = async {
            file.write_all(&msg.payload).await?;
            file.flush().await?;
            Ok::<_, std::io::Error>(())
        }
        .await
        .map_err(|e| {
            anyhow!(
                "JsonlWriter file write failed for '{}': {}",
                path.display(),
                e
            )
        });
        let _ = msg.ack.send(result);
    }

    debug!(path = %path.display(), "JsonlWriter actor shutting down (all senders dropped)");
}

/// Append-only JSONL writer for [`Event`] records.
///
/// Internally backed by a [`JsonlWriter`] actor so concurrent appends across
/// any number of clones are guaranteed to be line-atomic and ordered.
#[derive(Clone, Debug)]
pub struct EventWriter {
    inner: JsonlWriter,
}

impl EventWriter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            inner: JsonlWriter::new(path),
        }
    }

    /// Serialize one event and append it.
    pub async fn append(&self, event: &Event) -> Result<()> {
        let mut buf = serde_json::to_vec(event)
            .with_context(|| format!("failed to serialize event {}", event.id))?;
        buf.push(b'\n');
        self.inner
            .append_line(buf)
            .await
            .with_context(|| format!("failed to append event {}", event.id))?;
        debug!(event_id = %event.id, "Appended event");
        Ok(())
    }

    /// Serialize many events and append them as a single contiguous batch.
    ///
    /// The batch is sent as one message to the writer actor, so all events
    /// in the batch are guaranteed to land contiguously in the file with
    /// no interleaving from concurrent producers.
    pub async fn append_many(&self, events: &[Event]) -> Result<()> {
        let mut buffer = Vec::new();
        for event in events {
            serde_json::to_writer(&mut buffer, event)
                .with_context(|| format!("failed to serialize event {}", event.id))?;
            buffer.push(b'\n');
        }
        self.inner
            .append_line(buffer)
            .await
            .with_context(|| format!("failed to append batch of {} events", events.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn concurrent_producers_do_not_interleave_lines() {
        // Headline correctness claim for the mpsc-actor design: N concurrent
        // tasks each writing one line must produce exactly N intact lines.
        // Previously (open-write-close per call on a Clone'd writer) this
        // could interleave on filesystems without strong O_APPEND guarantees.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("concurrent.jsonl");
        let writer = Arc::new(JsonlWriter::new(&path));

        const N: usize = 64;
        let payload_len = 8192;
        let mut handles = Vec::with_capacity(N);
        for i in 0..N {
            let writer = Arc::clone(&writer);
            handles.push(tokio::spawn(async move {
                let body = "x".repeat(payload_len);
                let mut line = format!("{{\"i\":{},\"body\":\"{}\"}}", i, body).into_bytes();
                line.push(b'\n');
                writer.append_line(line).await.unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        // Drop the last sender so the actor flushes and exits.
        drop(writer);
        // Give the actor a chance to drain after sender close.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let mut lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), N, "expected {} lines, got {}", N, lines.len());

        // Every line must parse as JSON and contain `{"i":<int>,"body":"..."}`
        lines.sort();
        for line in lines {
            let v: serde_json::Value =
                serde_json::from_str(line).expect("each line must be intact JSON");
            assert!(v.get("i").and_then(|v| v.as_u64()).is_some());
            let body = v.get("body").and_then(|v| v.as_str()).unwrap();
            assert_eq!(
                body.len(),
                payload_len,
                "body payload must not be truncated"
            );
        }
    }

    #[tokio::test]
    async fn open_failure_surfaces_to_every_caller() {
        // Pointing JsonlWriter at a path that cannot be opened (a directory)
        // must NOT hang callers. Each append should resolve to Err.
        let dir = TempDir::new().unwrap();
        let writer = JsonlWriter::new(dir.path()); // a directory, not a file
        let result = writer.append_line(b"hello\n".to_vec()).await;
        assert!(
            result.is_err(),
            "append against a directory path must error"
        );
    }
}
