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
    #[serde(default, alias = "total_spawns")]
    pub total_team_runs: u64,
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
        m.total_team_runs = 5;
        m.save(&path).await.unwrap();
        let loaded = Metrics::load_or_default(&path).await.unwrap();
        assert_eq!(loaded.total_team_runs, 5);
    }

    #[tokio::test]
    async fn test_metrics_record() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("metrics.json");
        record(&path, |m| m.total_team_runs += 1).await.unwrap();
        let m = Metrics::load_or_default(&path).await.unwrap();
        assert_eq!(m.total_team_runs, 1);
    }

    #[tokio::test]
    async fn test_metrics_loads_legacy_total_spawns() {
        let legacy = serde_json::json!({
            "version": 1,
            "created_at": "2026-05-11T00:00:00Z",
            "updated_at": "2026-05-11T00:00:00Z",
            "total_spawns": 7,
            "total_shutdowns": 0,
            "total_tasks_created": 0,
            "total_tasks_completed": 0,
            "total_tasks_failed": 0,
            "total_ask_calls": 0,
            "total_ask_errors": 0,
            "total_autopilot_runs": 0,
            "total_ralph_runs": 0
        });

        let loaded: Metrics = serde_json::from_value(legacy).unwrap();

        assert_eq!(loaded.total_team_runs, 7);
    }
}
