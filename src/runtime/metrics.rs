use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_spawns: u64,
    pub total_shutdowns: u64,
    pub total_tasks_created: u64,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub total_ask_calls: u64,
    pub total_ask_errors: u64,
    pub total_autopilot_runs: u64,
    pub total_ralph_runs: u64,
}

impl Metrics {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            updated_at: now,
            ..Default::default()
        }
    }

    pub async fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = tokio::fs::read_to_string(path).await?;
        let m: Metrics = serde_json::from_str(&raw)
            .with_context(|| format!("parse metrics {}", path.display()))?;
        Ok(m)
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        let out = serde_json::to_vec_pretty(&self)?;
        crate::runtime::atomic::atomic_write(path, &out).await?;
        Ok(())
    }
}

pub async fn record(metrics_path: &Path, op: impl FnOnce(&mut Metrics)) -> Result<()> {
    let mut m = Metrics::load_or_default(metrics_path).await?;
    op(&mut m);
    m.updated_at = Utc::now();
    m.save(metrics_path).await?;
    info!(path = %metrics_path.display(), "Metrics updated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("metrics.json");
        let mut m = Metrics::new();
        m.total_spawns = 5;
        m.save(&path).await.unwrap();
        let loaded = Metrics::load_or_default(&path).await.unwrap();
        assert_eq!(loaded.total_spawns, 5);
    }

    #[tokio::test]
    async fn test_metrics_record() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("metrics.json");
        record(&path, |m| m.total_spawns += 1).await.unwrap();
        let m = Metrics::load_or_default(&path).await.unwrap();
        assert_eq!(m.total_spawns, 1);
    }
}
