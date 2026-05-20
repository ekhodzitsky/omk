use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

use super::sidecar;
use crate::runtime::goal::state::{GOAL_PROOF_FILE, GOAL_TASK_GRAPH_FILE};

pub(crate) async fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut value = serde_json::to_value(value)?;
    value = crate::wire::protocol::redact_wire_secrets(&value);
    enrich_goal_json_artifact(path, &mut value).await?;

    if let Some(goal_id) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
        if let Some(db) = crate::runtime::db::global_db() {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name == GOAL_PROOF_FILE {
                if let Ok(proof) = serde_json::from_value::<super::super::proof::GoalProof>(value.clone()) {
                    super::super::state::db_store::save_proof_to_db(&db, &proof).await?;
                    return Ok(());
                }
            }
            if file_name == GOAL_TASK_GRAPH_FILE {
                if let Ok(graph) = serde_json::from_value::<super::super::task_graph::GoalTaskGraph>(value.clone()) {
                    super::super::state::db_store::save_task_graph_to_db(&db, &graph).await?;
                    return Ok(());
                }
            }
        }
    }

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
            crate::runtime::goal::task_graph::preserve_delivery_metadata_in_value(goal_dir, value)
                .await
        }
        GOAL_PROOF_FILE => attach_delivery_metadata_to_proof_value(goal_dir, value).await,
        _ => Ok(()),
    }
}

async fn attach_delivery_metadata_to_proof_value(
    goal_dir: &Path,
    proof_value: &mut Value,
) -> Result<()> {
    let delivery_metadata =
        crate::runtime::goal::task_graph::load_task_delivery_metadata(goal_dir).await?;
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
