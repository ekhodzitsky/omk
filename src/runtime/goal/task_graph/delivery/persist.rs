use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::metadata::{
    GoalTaskDeliveryMetadata, GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryRecord,
};
use crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE;

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

pub(crate) async fn load_task_delivery_metadata(goal_dir: &Path) -> Result<Vec<Value>> {
    let records = load_goal_task_delivery_records(goal_dir).await?;
    records
        .into_iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to serialize goal task delivery metadata")
}

pub(super) async fn load_task_graph_value(goal_dir: &Path) -> Result<Value> {
    let path = task_graph_path(goal_dir);
    let json = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
    parse_task_graph_value(&path, &json)
}

pub(super) fn task_graph_path(goal_dir: &Path) -> PathBuf {
    goal_dir.join(GOAL_TASK_GRAPH_FILE)
}

pub(super) fn task_ids_in_value(value: &Value) -> HashSet<&str> {
    value
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|task| task.get("id").and_then(Value::as_str))
        .collect()
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

fn parse_task_graph_value(path: &Path, json: &str) -> Result<Value> {
    serde_json::from_str(json)
        .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))
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
