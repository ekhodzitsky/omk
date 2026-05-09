#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::estimator::CostEstimate;

/// Tracked cost for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCost {
    pub session_type: String,
    pub name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub estimate: CostEstimate,
    pub actual_usd: Option<f64>,
}

/// Persistent cost tracker.
pub struct CostTracker {
    path: std::path::PathBuf,
}

impl CostTracker {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            path: state_dir.join("costs.json"),
        }
    }

    pub async fn load(&self) -> Result<Vec<SessionCost>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = tokio::fs::read_to_string(&self.path).await?;
        let costs: Vec<SessionCost> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", self.path.display()))?;
        Ok(costs)
    }

    pub async fn save(&self, costs: &[SessionCost]) -> Result<()> {
        let json = serde_json::to_string_pretty(costs)?;
        crate::runtime::atomic::atomic_write(&self.path, json.as_bytes()).await?;
        Ok(())
    }

    pub async fn record(&self, cost: SessionCost) -> Result<()> {
        let mut costs = self.load().await?;
        costs.push(cost);
        self.save(&costs).await?;
        Ok(())
    }

    pub async fn total_estimated(&self) -> Result<f64> {
        let costs = self.load().await?;
        Ok(costs.iter().map(|c| c.estimate.estimated_usd).sum())
    }

    pub async fn report(&self) -> Result<String> {
        let costs = self.load().await?;
        if costs.is_empty() {
            return Ok("No cost data recorded yet.".to_string());
        }

        let total: f64 = costs.iter().map(|c| c.estimate.estimated_usd).sum();
        let by_type = costs
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, c| {
                *acc.entry(c.session_type.clone()).or_insert(0.0) += c.estimate.estimated_usd;
                acc
            });

        let mut report = "💰 OMK Cost Report\n".to_string();
        report.push_str(&format!("Total estimated: ~${:.4}\n\n", total));
        report.push_str("By session type:\n");
        for (t, amount) in by_type {
            report.push_str(&format!("  {:20} ${:.4}\n", t, amount));
        }
        report.push_str(&format!("\nSessions: {}\n", costs.len()));

        Ok(report)
    }
}
