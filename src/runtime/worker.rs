use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::info;

/// Specification for a single worker in a team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSpec {
    pub name: String,
    pub role: String,
    pub inbox: PathBuf,
    pub outbox: PathBuf,
    pub heartbeat: PathBuf,
}

impl WorkerSpec {
    pub async fn save(&self) -> Result<()> {
        let path = self.inbox.parent().unwrap().join("worker-spec.json");
        let json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, json).await?;
        info!(path = %path.display(), name = %self.name, "Saved worker spec");
        Ok(())
    }

    pub async fn load(worker_dir: &std::path::Path) -> Result<Self> {
        let path = worker_dir.join("worker-spec.json");
        let json = tokio::fs::read_to_string(&path).await?;
        let spec: Self = serde_json::from_str(&json)?;
        Ok(spec)
    }

    /// Write a task to the inbox
    pub async fn send_task(&self, task: &WorkerTask) -> Result<()> {
        let line = serde_json::to_string(task)?;
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.inbox)
            .await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        Ok(())
    }

    /// Read all completed results from outbox
    pub async fn read_results(&self) -> Result<Vec<WorkerResult>> {
        if !self.outbox.exists() {
            return Ok(vec![]);
        }
        let content = tokio::fs::read_to_string(&self.outbox).await?;
        let mut results = vec![];
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(line) {
                Ok(r) => results.push(r),
                Err(e) => tracing::warn!(line = %line, error = %e, "Failed to parse worker result"),
            }
        }
        Ok(results)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTask {
    pub id: String,
    pub task: String,
    pub acceptance_criteria: Vec<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub task_id: String,
    pub status: ResultStatus,
    pub summary: String,
    pub artifacts: Vec<String>,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResultStatus {
    Success,
    Partial,
    Failed,
}
