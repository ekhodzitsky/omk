use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::super::state::GOAL_TASK_GRAPH_FILE;

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
    let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
    let json = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
    let value: Value = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
    Ok(collect_delivery_metadata(&value))
}

async fn load_delivery_by_task_id(goal_dir: &Path) -> Result<HashMap<String, Value>> {
    let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
    match tokio::fs::read_to_string(&path).await {
        Ok(json) => {
            let value: Value = serde_json::from_str(&json)
                .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
            Ok(collect_delivery_by_task_id(&value))
        }
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

fn collect_delivery_metadata(value: &Value) -> Vec<Value> {
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
            let mut record = delivery.clone();
            record.insert("task_id".to_string(), Value::String(task_id.to_string()));
            Some(Value::Object(record))
        })
        .collect()
}
