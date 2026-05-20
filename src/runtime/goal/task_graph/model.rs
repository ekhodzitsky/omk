use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalTaskStatus {
    Pending,
    Blocked,
    Done,
}

impl std::fmt::Display for GoalTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalTaskStatus::Pending => "pending",
            GoalTaskStatus::Blocked => "blocked",
            GoalTaskStatus::Done => "done",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskEvidence {
    pub kind: String,
    pub path: PathBuf,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: GoalTaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub evidence: Vec<GoalTaskEvidence>,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub dependencies: Vec<String>,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub risk: String,
    pub acceptance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskGraph {
    pub version: u32,
    pub goal_id: String,
    pub generated_at: DateTime<Utc>,
    pub tasks: Vec<GoalTask>,
}

impl GoalTaskGraph {
    pub async fn load(goal_dir: &Path) -> Result<Self> {
        if let Some(db) = crate::runtime::db::global_db() {
            if let Some(goal_id) = goal_dir.file_name().and_then(|n| n.to_str()) {
                if let Some(graph) =
                    crate::runtime::goal::state::db_store::load_task_graph_from_db(&db, goal_id)
                        .await?
                {
                    graph.validate()?;
                    return Ok(graph);
                }
            }
        }

        let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
        let graph: Self = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
        graph
            .validate()
            .with_context(|| format!("Invalid goal task graph: {}", path.display()))?;
        Ok(graph)
    }

    pub fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        if self.version == 0 {
            errors.push("task graph version must be greater than zero".to_string());
        }
        if self.goal_id.trim().is_empty() {
            errors.push("task graph goal_id must not be empty".to_string());
        }
        if self.tasks.is_empty() {
            errors.push("task graph must contain at least one task".to_string());
        }

        let mut task_ids = HashSet::new();
        for task in &self.tasks {
            let task_id = task.id.trim();
            if task_id.is_empty() {
                errors.push("task id must not be empty".to_string());
                continue;
            }
            if !task_ids.insert(task.id.as_str()) {
                errors.push(format!("duplicate task id: {}", task.id));
            }
            if task.title.trim().is_empty() {
                errors.push(format!("task {} title must not be empty", task.id));
            }
            if task.description.trim().is_empty() {
                errors.push(format!("task {} description must not be empty", task.id));
            }
            if task.acceptance.is_empty() {
                errors.push(format!(
                    "task {} must define at least one acceptance criterion",
                    task.id
                ));
            }
        }

        for task in &self.tasks {
            for dependency in &task.dependencies {
                if dependency == &task.id {
                    errors.push(format!("task {} cannot depend on itself", task.id));
                } else if !task_ids.contains(dependency.as_str()) {
                    errors.push(format!(
                        "task {} depends on missing task {}",
                        task.id, dependency
                    ));
                }
            }
        }

        if self.contains_dependency_cycle() {
            errors.push("task graph contains a dependency cycle".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }

    fn contains_dependency_cycle(&self) -> bool {
        let tasks_by_id: HashMap<&str, &GoalTask> = self
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task))
            .collect();
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        self.tasks.iter().any(|task| {
            dependency_cycle_from(task.id.as_str(), &tasks_by_id, &mut visiting, &mut visited)
        })
    }
}

fn dependency_cycle_from<'a>(
    task_id: &'a str,
    tasks_by_id: &HashMap<&'a str, &'a GoalTask>,
    visiting: &mut HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    if visited.contains(task_id) {
        return false;
    }
    if !visiting.insert(task_id) {
        return true;
    }

    if let Some(task) = tasks_by_id.get(task_id) {
        for dependency in &task.dependencies {
            if tasks_by_id.contains_key(dependency.as_str())
                && dependency_cycle_from(dependency.as_str(), tasks_by_id, visiting, visited)
            {
                return true;
            }
        }
    }

    visiting.remove(task_id);
    visited.insert(task_id);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, dependencies: &[&str]) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status: GoalTaskStatus::Pending,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: dependencies
                .iter()
                .map(|dependency| dependency.to_string())
                .collect(),
            read_set: Vec::new(),
            write_set: Vec::new(),
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
        GoalTaskGraph {
            version: 1,
            goal_id: "goal-test".to_string(),
            generated_at: Utc::now(),
            tasks,
        }
    }

    #[test]
    fn validate_accepts_dependency_dag() {
        let graph = graph(vec![
            task("goal-intake", &[]),
            task("goal-plan", &["goal-intake"]),
            task("goal-verify", &["goal-plan"]),
        ]);

        graph.validate().expect("valid graph should pass");
    }

    #[test]
    fn validate_rejects_duplicate_task_ids() {
        let graph = graph(vec![task("goal-intake", &[]), task("goal-intake", &[])]);

        let err = graph.validate().expect_err("duplicate ids must fail");

        assert!(
            err.to_string().contains("duplicate task id: goal-intake"),
            "{err}"
        );
    }

    #[test]
    fn validate_rejects_unknown_dependencies() {
        let graph = graph(vec![task("goal-verify", &["goal-plan"])]);

        let err = graph
            .validate()
            .expect_err("unknown dependencies must fail");

        assert!(
            err.to_string()
                .contains("task goal-verify depends on missing task goal-plan"),
            "{err}"
        );
    }

    #[test]
    fn validate_rejects_dependency_cycles() {
        let graph = graph(vec![
            task("goal-a", &["goal-c"]),
            task("goal-b", &["goal-a"]),
            task("goal-c", &["goal-b"]),
        ]);

        let err = graph.validate().expect_err("dependency cycles must fail");

        assert!(
            err.to_string()
                .contains("task graph contains a dependency cycle"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn load_defaults_retry_and_lease_metadata_for_legacy_graph() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": Utc::now(),
            "tasks": [
                {
                    "id": "goal-intake",
                    "title": "Task goal-intake",
                    "description": "Task goal-intake description",
                    "status": "pending",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": [],
                    "risk": "low",
                    "acceptance": ["Task goal-intake acceptance"]
                }
            ]
        });
        tokio::fs::write(
            tmp.path().join(GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&graph_json).expect("json"),
        )
        .await
        .expect("write legacy graph");

        let graph = GoalTaskGraph::load(tmp.path())
            .await
            .expect("legacy graph should load");

        assert_eq!(graph.tasks[0].retry_count, 0);
        assert_eq!(graph.tasks[0].max_retries, 0);
        assert!(graph.tasks[0].lease_expires_at.is_none());
    }
}
