use anyhow::{Context, Result};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::super::state::GOAL_TASK_GRAPH_FILE;
use super::super::worktree::GoalWorktreePlan;

pub(crate) async fn ensure_worktree_delivery_targets(
    goal_dir: &Path,
    plans: &[GoalWorktreePlan],
) -> Result<()> {
    let value = load_task_graph_value(goal_dir).await?;
    let task_ids = task_ids_in_value(&value);
    for plan in plans {
        if !task_ids.contains(plan.task_id.as_str()) {
            anyhow::bail!(
                "cannot record goal worktree delivery metadata: task {} not found in {}",
                plan.task_id,
                task_graph_path(goal_dir).display()
            );
        }
    }
    Ok(())
}

pub(crate) async fn record_worktree_delivery_metadata(
    goal_dir: &Path,
    plan: &GoalWorktreePlan,
) -> Result<()> {
    update_goal_task_delivery_metadata(
        goal_dir,
        &plan.task_id,
        GoalTaskDeliveryMetadataUpdate {
            branch: Some(plan.branch_name.clone()),
            worktree_path: Some(plan.worktree_path.clone()),
            ..GoalTaskDeliveryMetadataUpdate::default()
        },
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalTaskDeliveryStatus {
    Planned,
    InProgress,
    Blocked,
    ReadyForReview,
    Delivered,
    Merged,
    Other(String),
}

impl GoalTaskDeliveryStatus {
    pub fn as_str(&self) -> &str {
        match self {
            GoalTaskDeliveryStatus::Planned => "planned",
            GoalTaskDeliveryStatus::InProgress => "in_progress",
            GoalTaskDeliveryStatus::Blocked => "blocked",
            GoalTaskDeliveryStatus::ReadyForReview => "ready_for_review",
            GoalTaskDeliveryStatus::Delivered => "delivered",
            GoalTaskDeliveryStatus::Merged => "merged",
            GoalTaskDeliveryStatus::Other(status) => status.as_str(),
        }
    }
}

impl From<String> for GoalTaskDeliveryStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "planned" => GoalTaskDeliveryStatus::Planned,
            "in_progress" => GoalTaskDeliveryStatus::InProgress,
            "blocked" => GoalTaskDeliveryStatus::Blocked,
            "ready_for_review" => GoalTaskDeliveryStatus::ReadyForReview,
            "delivered" => GoalTaskDeliveryStatus::Delivered,
            "merged" => GoalTaskDeliveryStatus::Merged,
            _ => GoalTaskDeliveryStatus::Other(value),
        }
    }
}

impl Serialize for GoalTaskDeliveryStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GoalTaskDeliveryStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(String::deserialize(deserializer)?.into())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GoalTaskDeliveryMetadata {
    pub owner: Option<String>,
    pub write_scope: Vec<String>,
    pub branch: Option<String>,
    pub worktree_path: Option<PathBuf>,
    pub pr_url: Option<String>,
    pub commit_sha: Option<String>,
    pub verification_summary: Option<String>,
    pub status: Option<GoalTaskDeliveryStatus>,
    pub extra: Map<String, Value>,
}

impl GoalTaskDeliveryMetadata {
    pub fn from_value(value: &Value) -> Self {
        let Some(object) = value.as_object() else {
            return Self::default();
        };
        let mut extra = object.clone();
        let owner = take_string(&mut extra, "owner");
        let write_scope = take_string_array(&mut extra, "write_scope").unwrap_or_default();
        let branch = take_string(&mut extra, "branch");
        let worktree_path = take_string(&mut extra, "worktree_path").map(PathBuf::from);
        let pr_url = take_string(&mut extra, "pr_url");
        let commit_sha = take_string(&mut extra, "commit_sha");
        let verification_summary = take_string(&mut extra, "verification_summary");
        let status = take_string(&mut extra, "status").map(GoalTaskDeliveryStatus::from);

        Self {
            owner,
            write_scope,
            branch,
            worktree_path,
            pr_url,
            commit_sha,
            verification_summary,
            status,
            extra,
        }
    }

    pub fn to_value(&self) -> Value {
        let mut object = self.extra.clone();
        insert_string(&mut object, "owner", self.owner.as_deref());
        if !self.write_scope.is_empty() {
            object.insert(
                "write_scope".to_string(),
                serde_json::json!(self.write_scope),
            );
        }
        insert_string(&mut object, "branch", self.branch.as_deref());
        let worktree_path = self
            .worktree_path
            .as_ref()
            .map(|path| path.display().to_string());
        insert_string(&mut object, "worktree_path", worktree_path.as_deref());
        insert_string(&mut object, "pr_url", self.pr_url.as_deref());
        insert_string(&mut object, "commit_sha", self.commit_sha.as_deref());
        insert_string(
            &mut object,
            "verification_summary",
            self.verification_summary.as_deref(),
        );
        insert_string(
            &mut object,
            "status",
            self.status.as_ref().map(GoalTaskDeliveryStatus::as_str),
        );
        Value::Object(object)
    }

    pub fn merge_update(&mut self, update: GoalTaskDeliveryMetadataUpdate) {
        replace_if_some(&mut self.owner, update.owner);
        if let Some(write_scope) = update.write_scope {
            self.write_scope = write_scope;
        }
        replace_if_some(&mut self.branch, update.branch);
        replace_if_some(&mut self.worktree_path, update.worktree_path);
        replace_if_some(&mut self.pr_url, update.pr_url);
        replace_if_some(&mut self.commit_sha, update.commit_sha);
        replace_if_some(&mut self.verification_summary, update.verification_summary);
        replace_if_some(&mut self.status, update.status);
        self.extra.extend(update.extra);
    }

    fn is_empty(&self) -> bool {
        matches!(self.to_value(), Value::Object(object) if object.is_empty())
    }
}

impl Serialize for GoalTaskDeliveryMetadata {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GoalTaskDeliveryMetadata {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if !value.is_object() {
            return Err(D::Error::custom("delivery metadata must be a JSON object"));
        }
        Ok(Self::from_value(&value))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GoalTaskDeliveryMetadataUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_scope: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<GoalTaskDeliveryStatus>,
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GoalTaskDeliveryRecord {
    pub task_id: String,
    #[serde(flatten)]
    pub metadata: GoalTaskDeliveryMetadata,
}

impl Serialize for GoalTaskDeliveryRecord {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = self.metadata.to_value();
        if let Some(object) = value.as_object_mut() {
            object.insert("task_id".to_string(), Value::String(self.task_id.clone()));
        }
        value.serialize(serializer)
    }
}

pub async fn load_goal_task_delivery_records(
    goal_dir: &Path,
) -> Result<Vec<GoalTaskDeliveryRecord>> {
    let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
    let json = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
    let value: Value = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
    Ok(collect_delivery_records(&value))
}

pub async fn read_goal_task_delivery_metadata(
    goal_dir: &Path,
    task_id: &str,
) -> Result<Option<GoalTaskDeliveryMetadata>> {
    let records = load_goal_task_delivery_records(goal_dir).await?;
    Ok(records
        .into_iter()
        .find(|record| record.task_id == task_id)
        .map(|record| record.metadata))
}

pub async fn update_goal_task_delivery_metadata(
    goal_dir: &Path,
    task_id: &str,
    update: GoalTaskDeliveryMetadataUpdate,
) -> Result<GoalTaskDeliveryMetadata> {
    let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
    let json = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
    let mut value: Value = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
    let metadata = update_delivery_metadata_in_value(&mut value, task_id, update)?;
    let json = serde_json::to_vec_pretty(&value)?;
    crate::runtime::atomic::atomic_write(&path, &json).await?;
    Ok(metadata)
}

pub(crate) async fn preserve_delivery_metadata_in_value(
    goal_dir: &Path,
    graph_value: &mut Value,
) -> Result<()> {
    let delivery_by_task_id = load_delivery_by_task_id(goal_dir).await?;
    if let Some(tasks) = graph_value.get_mut("tasks").and_then(Value::as_array_mut) {
        for task in tasks {
            let Some(task_id) = task.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(delivery) = delivery_by_task_id.get(task_id) else {
                continue;
            };
            if let Some(task_object) = task.as_object_mut() {
                task_object.insert("delivery".to_string(), delivery.clone());
            }
        }
    }

    Ok(())
}

async fn load_task_graph_value(goal_dir: &Path) -> Result<Value> {
    let path = task_graph_path(goal_dir);
    let json = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
    parse_task_graph_value(&path, &json)
}

fn task_graph_path(goal_dir: &Path) -> PathBuf {
    goal_dir.join(GOAL_TASK_GRAPH_FILE)
}

fn parse_task_graph_value(path: &Path, json: &str) -> Result<Value> {
    serde_json::from_str(json)
        .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))
}

fn task_ids_in_value(value: &Value) -> HashSet<&str> {
    value
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|task| task.get("id").and_then(Value::as_str))
        .collect()
}

pub(crate) async fn load_task_delivery_metadata(goal_dir: &Path) -> Result<Vec<Value>> {
    let records = load_goal_task_delivery_records(goal_dir).await?;
    records
        .into_iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to serialize goal task delivery metadata")
}

async fn load_delivery_by_task_id(goal_dir: &Path) -> Result<HashMap<String, Value>> {
    let path = task_graph_path(goal_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(json) => Ok(collect_delivery_by_task_id(&parse_task_graph_value(
            &path, &json,
        )?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to read goal task graph: {}", path.display())),
    }
}

fn collect_delivery_by_task_id(value: &Value) -> HashMap<String, Value> {
    value
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|task| {
            let task_id = task.get("id").and_then(Value::as_str)?;
            let delivery = task.get("delivery")?.as_object()?;
            if delivery.is_empty() {
                return None;
            }
            Some((task_id.to_string(), Value::Object(delivery.clone())))
        })
        .collect()
}

fn collect_delivery_records(value: &Value) -> Vec<GoalTaskDeliveryRecord> {
    value
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|task| {
            let task_id = task.get("id").and_then(Value::as_str)?;
            let delivery = task.get("delivery")?.as_object()?;
            if delivery.is_empty() {
                return None;
            }
            Some(GoalTaskDeliveryRecord {
                task_id: task_id.to_string(),
                metadata: GoalTaskDeliveryMetadata::from_value(&Value::Object(delivery.clone())),
            })
        })
        .collect()
}

fn update_delivery_metadata_in_value(
    graph_value: &mut Value,
    task_id: &str,
    update: GoalTaskDeliveryMetadataUpdate,
) -> Result<GoalTaskDeliveryMetadata> {
    let tasks = graph_value
        .get_mut("tasks")
        .and_then(Value::as_array_mut)
        .context("goal task graph must contain a tasks array")?;
    let task = tasks
        .iter_mut()
        .find(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
        .with_context(|| format!("goal task graph does not contain task id: {task_id}"))?;
    let task_object = task
        .as_object_mut()
        .with_context(|| format!("goal task {task_id} must be a JSON object"))?;
    let current = task_object
        .get("delivery")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut metadata = GoalTaskDeliveryMetadata::from_value(&current);
    metadata.merge_update(update);

    if metadata.is_empty() {
        task_object.remove("delivery");
    } else {
        task_object.insert("delivery".to_string(), metadata.to_value());
    }

    Ok(metadata)
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    let value = object.get(key)?.as_str()?.to_string();
    object.remove(key);
    Some(value)
}

fn take_string_array(object: &mut Map<String, Value>, key: &str) -> Option<Vec<String>> {
    let values = serde_json::from_value(object.get(key)?.clone()).ok()?;
    object.remove(key);
    Some(values)
}

fn insert_string(object: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        object.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn replace_if_some<T>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}
