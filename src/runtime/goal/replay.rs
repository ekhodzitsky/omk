use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_gaps: Vec<String>,
    pub duplicate_events: usize,
    pub parse_failures: usize,
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

    let mut known_gaps = Vec::new();
    let mut recovery_status = None;
    let mut duplicate_events = 0usize;

    // Load task graph; treat failure as a recoverable known gap rather than a hard error.
    let task_graph_summary = match super::GoalTaskGraph::load(&state.state_dir).await {
        Ok(task_graph) => summarize_task_graph(&task_graph),
        Err(error) => {
            let gap = format!(
                "Task graph could not be loaded for replay: {}",
                error.root_cause()
            );
            known_gaps.push(gap.clone());
            recovery_status = Some(format!("partial: {gap}"));
            GoalTaskGraphSummary::default()
        }
    };

    // Load event log with tolerance for partial/corrupt trailing lines.
    let event_log = crate::runtime::config::resolve_event_log_for_read(&state.state_dir);
    let events = crate::runtime::events::EventReader::read_all(&event_log).await?;

    // Compute parse failures by comparing raw lines to parsed events.
    // EventReader::read_all silently skips malformed lines; we surface the count.
    let summary = crate::runtime::events::EventReader::summary(&event_log).await?;
    let parse_failures = summary.parse_failures;

    if parse_failures > 0 {
        known_gaps.push(format!(
            "Event log contains {parse_failures} malformed line(s) that were skipped during replay"
        ));
        recovery_status = Some(format!(
            "partial: {parse_failures} event line(s) could not be parsed"
        ));
    }

    // Deduplicate events deterministically: collapse exact JSON duplicates.
    // Two events are considered duplicates if their canonical JSON representation is identical.
    let mut seen = HashSet::new();
    let mut timeline = Vec::with_capacity(events.len());
    let mut dedup_failures = 0usize;
    for (index, event) in events.iter().enumerate() {
        let canonical = match serde_json::to_string(event) {
            Ok(s) => s,
            Err(error) => {
                dedup_failures += 1;
                known_gaps.push(format!(
                    "Event at index {index} could not be serialized for deduplication: {error}"
                ));
                continue;
            }
        };
        if !seen.insert(canonical) {
            duplicate_events += 1;
            continue;
        }
        timeline.push(GoalReplayEntry {
            index,
            ts: event.ts,
            kind: event_kind_label(&event.kind),
            actor: event.actor.clone(),
            summary: event_summary(event.payload.as_ref()),
        });
    }

    if dedup_failures > 0 {
        recovery_status = Some(format!(
            "partial: {dedup_failures} event(s) could not be deduplicated"
        ));
    }

    if duplicate_events > 0 {
        known_gaps.push(format!(
            "Event log contained {duplicate_events} duplicate event(s) that were collapsed deterministically"
        ));
    }

    let generated_at = timeline
        .last()
        .map(|entry| entry.ts)
        .unwrap_or(state.updated_at);

    if recovery_status.is_none() && !known_gaps.is_empty() {
        recovery_status = Some("recovered: replay completed with known gaps".to_string());
    }

    Ok(GoalReplay {
        version: 1,
        goal_id: state.goal_id,
        status: state.status,
        phase: state.phase,
        generated_at,
        event_count: timeline.len(),
        task_graph_summary,
        timeline,
        recovery_status,
        known_gaps,
        duplicate_events,
        parse_failures,
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
        "label",
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
