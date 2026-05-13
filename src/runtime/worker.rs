use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

/// Specification for a single worker in a team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSpec {
    pub name: String,
    pub role: String,
    pub inbox: PathBuf,
    pub outbox: PathBuf,
    pub heartbeat: PathBuf,
    #[serde(default)]
    pub project_dir: Option<PathBuf>,
}

impl WorkerSpec {
    pub async fn save(&self) -> Result<()> {
        let path = self
            .inbox
            .parent()
            .context("inbox path has no parent")?
            .join("worker-spec.json");
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
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
        crate::runtime::atomic::atomic_append_jsonl(&self.inbox, &line).await?;
        Ok(())
    }

    /// Write a result to the outbox
    pub async fn send_result(&self, result: &WorkerResult) -> Result<()> {
        let line = format!("{}\n", serde_json::to_string(result)?);
        crate::runtime::atomic::atomic_append(&self.outbox, line.as_bytes()).await?;
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
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub task_id: String,
    pub status: ResultStatus,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Success,
    Partial,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_spec_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: dir.path().join("inbox.jsonl"),
            outbox: dir.path().join("outbox.jsonl"),
            heartbeat: dir.path().join("heartbeat.json"),
            project_dir: None,
        };

        spec.save().await.unwrap();
        let loaded = WorkerSpec::load(dir.path()).await.unwrap();
        assert_eq!(loaded.name, "worker-0");
        assert_eq!(loaded.role, "coder");
    }

    #[tokio::test]
    async fn test_send_and_read_task() {
        let dir = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: dir.path().join("inbox.jsonl"),
            outbox: dir.path().join("outbox.jsonl"),
            heartbeat: dir.path().join("heartbeat.json"),
            project_dir: None,
        };

        let task = WorkerTask {
            id: "task-1".to_string(),
            task: "fix bug".to_string(),
            acceptance_criteria: vec!["tests pass".to_string()],
            context: None,
            budget_secs: None,
        };

        spec.send_task(&task).await.unwrap();

        let results = spec.read_results().await.unwrap();
        assert!(results.is_empty());

        // Simulate a result written by a worker
        let result = WorkerResult {
            task_id: "task-1".to_string(),
            status: ResultStatus::Success,
            summary: "done".to_string(),
            artifacts: vec![],
            elapsed_secs: 10,
        };
        let line = serde_json::to_string(&result).unwrap();
        tokio::fs::write(&spec.outbox, format!("{}\n", line))
            .await
            .unwrap();

        let results = spec.read_results().await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].task_id, "task-1");
        matches!(results[0].status, ResultStatus::Success);
    }

    #[tokio::test]
    async fn test_send_result_writes_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: dir.path().join("inbox.jsonl"),
            outbox: dir.path().join("outbox.jsonl"),
            heartbeat: dir.path().join("heartbeat.json"),
            project_dir: None,
        };

        let result = WorkerResult {
            task_id: "task-1".to_string(),
            status: ResultStatus::Success,
            summary: "done".to_string(),
            artifacts: vec!["file.rs".to_string()],
            elapsed_secs: 42,
        };

        spec.send_result(&result).await.unwrap();

        let content = tokio::fs::read_to_string(&spec.outbox).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["task_id"], "task-1");
        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["summary"], "done");
        assert_eq!(parsed["elapsed_secs"], 42);
    }

    #[tokio::test]
    async fn test_send_result_appends_multiple_lines() {
        let dir = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: dir.path().join("inbox.jsonl"),
            outbox: dir.path().join("outbox.jsonl"),
            heartbeat: dir.path().join("heartbeat.json"),
            project_dir: None,
        };

        spec.send_result(&WorkerResult {
            task_id: "task-1".to_string(),
            status: ResultStatus::Success,
            summary: "first".to_string(),
            artifacts: vec![],
            elapsed_secs: 1,
        })
        .await
        .unwrap();

        spec.send_result(&WorkerResult {
            task_id: "task-2".to_string(),
            status: ResultStatus::Failed,
            summary: "second".to_string(),
            artifacts: vec![],
            elapsed_secs: 2,
        })
        .await
        .unwrap();

        let content = tokio::fs::read_to_string(&spec.outbox).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("task-1"));
        assert!(lines[1].contains("task-2"));
    }
}
