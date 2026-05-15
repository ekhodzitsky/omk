use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

use crate::runtime::goal::state::{GOAL_PROOF_FILE, GOAL_TASK_GRAPH_FILE};
use super::sidecar;

pub(crate) async fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut value = serde_json::to_value(value)?;
    enrich_goal_json_artifact(path, &mut value).await?;
    let json = serde_json::to_string_pretty(&value)?;
    crate::runtime::atomic::atomic_write(path, json.as_bytes()).await
}

async fn enrich_goal_json_artifact(path: &Path, value: &mut Value) -> Result<()> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    let Some(goal_dir) = path.parent() else {
        return Ok(());
    };

    match file_name {
        GOAL_TASK_GRAPH_FILE => {
            crate::runtime::goal::task_graph::preserve_delivery_metadata_in_value(goal_dir, value).await
        }
        GOAL_PROOF_FILE => attach_delivery_metadata_to_proof_value(goal_dir, value).await,
        _ => Ok(()),
    }
}

async fn attach_delivery_metadata_to_proof_value(
    goal_dir: &Path,
    proof_value: &mut Value,
) -> Result<()> {
    let delivery_metadata = crate::runtime::goal::task_graph::load_task_delivery_metadata(goal_dir).await?;
    sidecar::remember_goal_proof_delivery_metadata_for_value(
        proof_value,
        delivery_metadata.clone(),
    );
    if delivery_metadata.is_empty() {
        return Ok(());
    }
    if let Some(proof) = proof_value.as_object_mut() {
        proof.insert(
            "delivery_metadata".to_string(),
            Value::Array(delivery_metadata),
        );
    }
    Ok(())
}
