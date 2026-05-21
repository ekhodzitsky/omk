use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::types::{ClassificationSource, Intent};

/// Serialize access to the global telemetry file so concurrent
/// `append` / `compact_if_stale` operations never race on rename.
static TELEMETRY_LOCK: Mutex<()> = Mutex::const_new(());

pub fn telemetry_path() -> PathBuf {
    crate::runtime::config::state_dir().join("telemetry.jsonl")
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryRecord {
    pub ts: DateTime<Utc>,
    pub intent: Intent,
    pub confidence: f32,
    pub source: ClassificationSource,
    pub latency_ms: u32,
    pub prompt_hash: String,
    pub fallback: bool,
}

pub async fn append(record: TelemetryRecord) -> Result<()> {
    let _guard = TELEMETRY_LOCK.lock().await;
    let path = telemetry_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');
    file.write_all(&line).await?;
    file.flush().await?;
    Ok(())
}

pub fn prompt_hash_hex(prompt: &str) -> String {
    let mut hasher = DefaultHasher::new();
    prompt.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{:016x}", hash)
}

pub async fn compact_if_stale(retain_days: i64) -> Result<()> {
    let _guard = TELEMETRY_LOCK.lock().await;
    let path = telemetry_path();
    if !path.exists() {
        return Ok(());
    }
    let meta = tokio::fs::metadata(&path).await?;
    if meta.len() == 0 {
        return Ok(());
    }
    let contents = tokio::fs::read_to_string(&path).await?;
    let cutoff = Utc::now() - chrono::Duration::days(retain_days);
    let mut keep_lines: Vec<&str> = Vec::new();
    let mut dropped_any = false;
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let ts = match extract_ts(line) {
            Some(ts) => ts,
            None => {
                keep_lines.push(line);
                continue;
            }
        };
        if ts >= cutoff {
            keep_lines.push(line);
        } else {
            dropped_any = true;
        }
    }
    if !dropped_any {
        return Ok(());
    }
    let temp_path = path.with_extension("jsonl.tmp");
    let mut temp = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp_path)
        .await?;
    for line in &keep_lines {
        temp.write_all(line.as_bytes()).await?;
        temp.write_all(b"\n").await?;
    }
    temp.flush().await?;
    drop(temp);
    tokio::fs::rename(&temp_path, &path).await?;
    Ok(())
}

fn extract_ts(line: &str) -> Option<DateTime<Utc>> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    value.get("ts")?.as_str()?.parse().ok()
}

pub async fn write_engine_event(session_dir: &Path, event: &serde_json::Value) -> Result<()> {
    let path = session_dir.join("engine-events.jsonl");
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    let mut line = serde_json::to_vec(event)?;
    line.push(b'\n');
    file.write_all(&line).await?;
    file.flush().await?;
    Ok(())
}
