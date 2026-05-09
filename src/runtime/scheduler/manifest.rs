use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

use super::task::{Task, TaskId};

/// Central manifest for a single run (team, autopilot, ralph, ultrawork).
/// Stored on disk so it survives process restarts and can be inspected later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub mode: String,
    pub created_at: DateTime<Utc>,
    pub project_dir: PathBuf,
    pub tasks: Vec<Task>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

impl RunManifest {
    pub fn new(run_id: impl Into<String>, mode: impl Into<String>, project_dir: &Path) -> Self {
        Self {
            run_id: run_id.into(),
            mode: mode.into(),
            created_at: Utc::now(),
            project_dir: project_dir.to_path_buf(),
            tasks: Vec::new(),
            description: String::new(),
            ended_at: None,
            success: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_tasks(mut self, tasks: Vec<Task>) -> Self {
        self.tasks = tasks;
        self
    }

    /// Directory where this run's state and events are stored.
    pub fn run_dir(&self) -> PathBuf {
        crate::runtime::config::state_dir()
            .join("runs")
            .join(&self.run_id)
    }

    /// Path to the manifest file.
    pub fn manifest_path(&self) -> PathBuf {
        self.run_dir().join("manifest.json")
    }

    /// Path to append-only events log.
    pub fn events_path(&self) -> PathBuf {
        self.run_dir().join("events.jsonl")
    }

    /// Path to task state snapshot.
    pub fn tasks_path(&self) -> PathBuf {
        self.run_dir().join("tasks.jsonl")
    }

    /// Initialize the run directory and save the initial manifest.
    pub async fn init(&self) -> Result<()> {
        let run_dir = self.run_dir();
        tokio::fs::create_dir_all(&run_dir).await?;

        let manifest_json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&self.manifest_path(), manifest_json.as_bytes())
            .await?;

        // Touch events and tasks files
        tokio::fs::File::create(self.events_path()).await?;
        tokio::fs::File::create(self.tasks_path()).await?;

        info!(run_id = %self.run_id, dir = %run_dir.display(), "Run manifest initialized");
        Ok(())
    }

    /// Update the manifest on disk (e.g., after adding tasks or completing).
    pub async fn save(&self) -> Result<()> {
        let manifest_json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&self.manifest_path(), manifest_json.as_bytes())
            .await?;
        Ok(())
    }

    /// Append a single event to the events log.
    pub async fn append_event(&self, event: &RunEvent) -> Result<()> {
        let line = serde_json::to_string(event)?;
        let path = self.events_path();
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    /// Snapshot current task states to tasks.jsonl.
    pub async fn snapshot_tasks(&self) -> Result<()> {
        let path = self.tasks_path();
        let mut lines = Vec::new();
        for task in &self.tasks {
            lines.push(serde_json::to_string(task)?);
        }
        let content = lines.join("\n");
        crate::runtime::atomic::atomic_write(&path, content.as_bytes()).await?;
        Ok(())
    }

    /// Mark the run as finished.
    pub async fn finish(&mut self, success: bool) -> Result<()> {
        self.ended_at = Some(Utc::now());
        self.success = Some(success);
        self.save().await?;
        Ok(())
    }

    /// Load a manifest from disk by run_id.
    pub async fn load(run_id: &str) -> Result<Option<Self>> {
        let path = crate::runtime::config::state_dir()
            .join("runs")
            .join(run_id)
            .join("manifest.json");
        if !path.exists() {
            return Ok(None);
        }
        let json = tokio::fs::read_to_string(&path).await?;
        let manifest: Self = serde_json::from_str(&json)?;
        Ok(Some(manifest))
    }

    /// List all run IDs in the state directory.
    pub async fn list_runs() -> Result<Vec<String>> {
        let runs_dir = crate::runtime::config::state_dir().join("runs");
        if !runs_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = tokio::fs::read_dir(&runs_dir).await?;
        let mut runs = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                runs.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        // Sort by name (which includes timestamp) descending
        runs.sort_by(|a, b| b.cmp(a));
        Ok(runs)
    }
}

/// A single event in the run's append-only log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvent {
    pub ts: DateTime<Utc>,
    pub run_id: String,
    pub event_type: EventType,
    pub task_id: Option<TaskId>,
    pub worker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    RunStarted,
    TaskCreated,
    TaskClaimed,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    TaskCancelled,
    LeaseRecovered,
    ConflictDetected,
    ToolInvoked,
    FileChanged,
    GateRun,
    RunFinished,
    Recovery,
}

impl RunEvent {
    pub fn new(run_id: impl Into<String>, event_type: EventType) -> Self {
        Self {
            ts: Utc::now(),
            run_id: run_id.into(),
            event_type,
            task_id: None,
            worker: None,
            message: None,
            extra: std::collections::HashMap::new(),
        }
    }

    pub fn with_task(mut self, task_id: impl Into<TaskId>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    pub fn with_worker(mut self, worker: impl Into<String>) -> Self {
        self.worker = Some(worker.into());
        self
    }

    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn manifest_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let manifest = RunManifest::new("run-2024-01-01", "team", tmp.path())
            .with_description("test run")
            .with_tasks(vec![Task::new("t1", "task 1")]);

        let json = serde_json::to_string(&manifest).unwrap();
        let restored: RunManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.run_id, "run-2024-01-01");
        assert_eq!(restored.mode, "team");
        assert_eq!(restored.tasks.len(), 1);
    }

    #[tokio::test]
    async fn manifest_init_and_load() {
        let tmp = TempDir::new().unwrap();
        let manifest = RunManifest::new("run-test-123", "autopilot", tmp.path());
        manifest.init().await.unwrap();

        let loaded = RunManifest::load("run-test-123").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.run_id, "run-test-123");
        assert_eq!(loaded.mode, "autopilot");
    }
}
