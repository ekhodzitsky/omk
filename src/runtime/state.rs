use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamState {
    pub name: String,
    pub task: String,
    pub created_at: DateTime<Utc>,
    pub worker_count: usize,
    pub worker_role: String,
    pub phase: TeamPhase,
    pub tasks: Vec<Task>,
    pub state_dir: std::path::PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeamPhase {
    Planning,
    Executing,
    Verifying,
    Fixing,
    Complete,
    Failed,
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
    pub fn new(name: &str, task: &str, state_dir: &Path, worker_count: usize, worker_role: &str) -> Self {
        Self {
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
        tokio::fs::write(&path, json).await?;
        info!(path = %path.display(), "Saved team state");
        Ok(())
    }

    pub async fn load(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join("team-state.json");
        let json = tokio::fs::read_to_string(&path).await?;
        let state: Self = serde_json::from_str(&json)?;
        Ok(state)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotState {
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
    pub task: String,
    pub prd: Prd,
    pub iteration: usize,
    pub max_iterations: usize,
    pub state_dir: std::path::PathBuf,
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
