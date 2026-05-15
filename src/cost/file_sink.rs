use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::sink::CostSink;
use super::types::SessionCost;

/// File-based cost sink using atomic JSON read/write.
///
/// Writes are performed via `crate::runtime::atomic::atomic_write` so readers
/// never see partially-written files.
pub struct JsonFileCostSink {
    path: PathBuf,
}

impl JsonFileCostSink {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl CostSink for JsonFileCostSink {
    async fn save(&self, costs: &[SessionCost]) -> Result<()> {
        let json = serde_json::to_string_pretty(costs)?;
        crate::runtime::atomic::atomic_write(&self.path, json.as_bytes()).await?;
        Ok(())
    }

    async fn load(&self) -> Result<Vec<SessionCost>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = tokio::fs::read_to_string(&self.path).await?;
        let costs: Vec<SessionCost> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", self.path.display()))?;
        Ok(costs)
    }
}
