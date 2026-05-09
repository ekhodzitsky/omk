use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamState {
    #[serde(default = "default_state_version")]
    pub version: u32,
    pub name: String,
    pub task: String,
    pub created_at: DateTime<Utc>,
    pub worker_count: usize,
    pub worker_role: String,
    pub phase: TeamPhase,
    pub tasks: Vec<Task>,
    pub state_dir: std::path::PathBuf,
}

pub fn default_state_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeamPhase {
    Planning,
    Executing,
    Verifying,
    Fixing,
    Complete,
    Failed,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub assigned_to: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Done,
    Failed,
}

impl TeamState {
    pub fn new(
        name: &str,
        task: &str,
        state_dir: &Path,
        worker_count: usize,
        worker_role: &str,
    ) -> Self {
        Self {
            version: 1,
            name: name.to_string(),
            task: task.to_string(),
            created_at: Utc::now(),
            worker_count,
            worker_role: worker_role.to_string(),
            phase: TeamPhase::Planning,
            tasks: vec![],
            state_dir: state_dir.to_path_buf(),
        }
    }

    pub fn state_file(&self) -> std::path::PathBuf {
        self.state_dir.join("team-state.json")
    }

    pub async fn save(&self) -> Result<()> {
        let path = self.state_file();
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
        info!(path = %path.display(), "Saved team state");
        Ok(())
    }

    pub async fn load(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join("team-state.json");
        crate::runtime::migrate::migrate_if_needed(&path).await?;
        let json = tokio::fs::read_to_string(&path).await?;
        let state: Self = serde_json::from_str(&json)?;
        Ok(state)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotState {
    #[serde(default = "default_state_version")]
    pub version: u32,
    pub task: String,
    pub phase: AutopilotPhase,
    pub plans_dir: std::path::PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutopilotPhase {
    Expansion,
    Planning,
    Execution,
    Qa,
    Validation,
    Cleanup,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RalphState {
    #[serde(default = "default_state_version")]
    pub version: u32,
    pub task: String,
    pub prd: Prd,
    pub iteration: usize,
    pub max_iterations: usize,
    pub state_dir: std::path::PathBuf,
    #[serde(default)]
    pub gate_results: Vec<crate::runtime::gates::GateResult>,
}

impl RalphState {
    pub fn state_file(&self) -> std::path::PathBuf {
        self.state_dir.join("ralph-state.json")
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        let path = self.state_file();
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
        Ok(())
    }

    pub async fn load(state_dir: &Path) -> anyhow::Result<Self> {
        let path = state_dir.join("ralph-state.json");
        crate::runtime::migrate::migrate_if_needed(&path).await?;
        let json = tokio::fs::read_to_string(&path).await?;
        let state: Self = serde_json::from_str(&json)?;
        Ok(state)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Prd {
    pub user_stories: Vec<UserStory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStory {
    pub id: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub status: StoryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StoryStatus {
    NotStarted,
    InProgress,
    Implemented,
    Verified,
    Failed,
}

/// Resolve a run ID (or "latest") to a state directory path and the resolved run ID.
pub async fn resolve_run(run_id: &str) -> Result<(PathBuf, String)> {
    if run_id == "latest" {
        // Try team runs first
        let team_runs_dir =
            crate::runtime::config::omk_state_dir().join(crate::runtime::config::TEAM_DIR);
        if team_runs_dir.exists() {
            let mut entries = tokio::fs::read_dir(&team_runs_dir).await?;
            let mut runs = vec![];
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    runs.push(entry.path());
                }
            }
            sort_runs_by_mtime_desc(&mut runs).await;
            if let Some(latest) = runs.first() {
                return Ok((
                    latest.clone(),
                    latest
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                ));
            }
        }

        // Try scheduler runs
        let runs_dir = crate::runtime::config::state_dir().join("runs");
        if runs_dir.exists() {
            let mut entries = tokio::fs::read_dir(&runs_dir).await?;
            let mut runs = vec![];
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    runs.push(entry.path());
                }
            }
            sort_runs_by_mtime_desc(&mut runs).await;
            if let Some(latest) = runs.first() {
                return Ok((
                    latest.clone(),
                    latest
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                ));
            }
        }

        anyhow::bail!("No runs found");
    }

    // Direct run ID lookup
    let team_dir = crate::runtime::config::omk_state_dir()
        .join(crate::runtime::config::TEAM_DIR)
        .join(run_id);
    if team_dir.exists() {
        return Ok((team_dir, run_id.to_string()));
    }

    let scheduler_dir = crate::runtime::config::state_dir()
        .join("runs")
        .join(run_id);
    if scheduler_dir.exists() {
        return Ok((scheduler_dir, run_id.to_string()));
    }

    anyhow::bail!("Run '{}' not found", run_id);
}

async fn sort_runs_by_mtime_desc(runs: &mut Vec<PathBuf>) {
    let mut with_mtime = Vec::with_capacity(runs.len());
    for path in runs.drain(..) {
        let mtime = tokio::fs::metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        with_mtime.push((path, mtime));
    }

    with_mtime.sort_by(|a, b| match (a.1, b.1) {
        (Some(ta), Some(tb)) => tb.cmp(&ta).then_with(|| b.0.cmp(&a.0)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.0.cmp(&a.0),
    });

    runs.extend(with_mtime.into_iter().map(|(path, _)| path));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_team_state_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let state = TeamState::new("test", "task", dir.path(), 3, "coder");
        state.save().await.unwrap();

        let loaded = TeamState::load(dir.path()).await.unwrap();
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.task, "task");
        assert_eq!(loaded.worker_count, 3);
        assert_eq!(loaded.worker_role, "coder");
        matches!(loaded.phase, TeamPhase::Planning);
    }
}
