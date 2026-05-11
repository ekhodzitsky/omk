use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;
use tracing::warn;

use crate::runtime::events::{Event, EventKind};

/// Event reader that tolerates partial or corrupt trailing lines.
pub struct EventReader;

impl EventReader {
    /// Read all valid events from a JSONL file.
    /// Skips lines that fail to parse and logs a warning for each.
    pub async fn read_all(path: &Path) -> Result<Vec<Event>> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut events = Vec::new();
        for (line_no, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Event>(line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    warn!(line = line_no + 1, error = %e, "Skipping malformed event line");
                }
            }
        }
        Ok(events)
    }

    /// Read events filtered by kind.
    pub async fn read_filtered(path: &Path, kinds: &[EventKind]) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| kinds.contains(&e.kind))
            .collect();
        Ok(filtered)
    }

    /// Read events for a specific worker.
    pub async fn read_for_worker(path: &Path, worker: &str) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| e.actor.as_deref() == Some(worker))
            .collect();
        Ok(filtered)
    }

    /// Read events for a specific task id.
    pub async fn read_for_task(path: &Path, task_id: &str) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| payload_string(e, "task_id").as_deref() == Some(task_id))
            .collect();
        Ok(filtered)
    }

    /// Read events for a specific gate id or gate name.
    pub async fn read_for_gate(path: &Path, gate: &str) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| {
                payload_string(e, "gate_id").as_deref() == Some(gate)
                    || payload_string(e, "name").as_deref() == Some(gate)
            })
            .collect();
        Ok(filtered)
    }

    /// Read events within a time range.
    pub async fn read_range(
        path: &Path,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| e.ts >= from && e.ts <= to)
            .collect();
        Ok(filtered)
    }

    /// Return a summary: total lines, valid events, parse failures.
    pub async fn summary(path: &Path) -> Result<EventLogSummary> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(EventLogSummary::default())
            }
            Err(e) => return Err(e.into()),
        };

        let mut summary = EventLogSummary {
            total_lines: content.lines().count(),
            ..Default::default()
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                summary.empty_lines += 1;
                continue;
            }
            match serde_json::from_str::<Event>(line) {
                Ok(_) => summary.valid_events += 1,
                Err(_) => summary.parse_failures += 1,
            }
        }
        Ok(summary)
    }
}

pub(crate) fn payload_string(event: &Event, key: &str) -> Option<String> {
    event.payload.as_ref()?.get(key).and_then(|value| {
        if let Some(text) = value.as_str() {
            Some(text.to_string())
        } else {
            value
                .get("0")
                .and_then(|inner| inner.as_str())
                .map(str::to_string)
        }
    })
}

#[derive(Debug, Clone, Default)]
pub struct EventLogSummary {
    pub total_lines: usize,
    pub valid_events: usize,
    pub parse_failures: usize,
    pub empty_lines: usize,
}
