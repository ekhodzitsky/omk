use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::state::{GoalPhase, GoalStatus};
use super::task_graph::{summarize_task_graph, GoalTaskGraphSummary};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalReplay {
    pub version: u32,
    pub goal_id: String,
    pub status: GoalStatus,
    pub phase: GoalPhase,
    pub generated_at: DateTime<Utc>,
    pub event_count: usize,
    pub task_graph_summary: GoalTaskGraphSummary,
    pub timeline: Vec<GoalReplayEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalReplayEntry {
    pub index: usize,
    pub ts: DateTime<Utc>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

pub async fn replay_goal(goal_id: &str) -> Result<GoalReplay> {
    let state = super::resolve_goal(goal_id).await?;
    let task_graph = super::GoalTaskGraph::load(&state.state_dir).await?;
    let event_log = crate::runtime::config::resolve_event_log_for_read(&state.state_dir);
    let events = crate::runtime::events::EventReader::read_all(&event_log).await?;
    let timeline = events
        .iter()
        .enumerate()
        .map(|(index, event)| GoalReplayEntry {
            index,
            ts: event.ts,
            kind: event_kind_label(&event.kind),
            actor: event.actor.clone(),
            summary: event_summary(event.payload.as_ref()),
        })
        .collect::<Vec<_>>();

    Ok(GoalReplay {
        version: 1,
        goal_id: state.goal_id,
        status: state.status,
        phase: state.phase,
        generated_at: Utc::now(),
        event_count: timeline.len(),
        task_graph_summary: summarize_task_graph(&task_graph),
        timeline,
    })
}

fn event_kind_label(kind: &crate::runtime::events::EventKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn event_summary(payload: Option<&Value>) -> Option<String> {
    let payload = payload?;
    let mut parts = Vec::new();
    for key in [
        "message",
        "reason",
        "status",
        "phase",
        "task_id",
        "gate_id",
        "name",
        "action",
        "source",
        "worker_id",
    ] {
        if let Some(value) = payload_value(payload, key) {
            parts.push(format!("{key}={value}"));
        }
    }

    if !parts.is_empty() {
        return Some(parts.join(", "));
    }

    if payload.as_object().is_some_and(|object| object.is_empty()) {
        None
    } else {
        Some(payload.to_string())
    }
}

fn payload_value(payload: &Value, key: &str) -> Option<String> {
    scalar_value(payload.get(key)?)
}

fn scalar_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Object(object) => object.get("0").and_then(scalar_value),
        Value::Array(values) => {
            let values = values.iter().filter_map(scalar_value).collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(","))
        }
        Value::Null => None,
    }
}
