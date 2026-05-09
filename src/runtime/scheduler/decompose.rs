use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::wire::client::{WireClient, WireMessage};
use crate::wire::protocol::{ClientInfo, Event, InitializeParams};

/// A subtask produced by lead decomposition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Subtask {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub read_set: Vec<String>,
    #[serde(default)]
    pub write_set: Vec<String>,
}

/// Decomposes a high-level task into parallel subtasks via a Kimi lead agent.
pub struct LeadDecomposer;

impl LeadDecomposer {
    /// Ask a lead agent to break `task` into `count` parallel subtasks.
    /// Returns `Err` on any failure so the caller can fallback.
    pub async fn decompose(task: &str, count: usize, kimi_bin: &str) -> Result<Vec<Subtask>> {
        let prompt = format!(
            "You are a task planner. Break down the following task into {count} parallel, non-overlapping subtasks that can be executed by independent workers.\n\nTask: {task}\n\nReturn ONLY a JSON array (no markdown, no explanation). Include conservative file path ownership hints when known; use empty arrays when paths are unknown:\n[{{\"id\":\"task-1\",\"description\":\"subtask 1...\",\"read_set\":[\"path/to/read\"],\"write_set\":[\"path/to/write\"]}},{{\"id\":\"task-2\",\"description\":\"subtask 2...\",\"read_set\":[],\"write_set\":[]}},...]"
        );
        let response = run_wire_prompt(&prompt, kimi_bin, "omk-lead-decomposer").await?;
        parse_subtasks(&response)
    }
}

/// Synthesis agent that combines worker results into a final summary.
pub struct SynthesisAgent;

impl SynthesisAgent {
    /// Ask a synthesis agent to produce a concise summary.
    pub async fn synthesize(prompt: &str, kimi_bin: &str) -> Result<String> {
        run_wire_prompt(prompt, kimi_bin, "omk-synthesis-agent").await
    }
}

async fn run_wire_prompt(prompt: &str, kimi_bin: &str, client_name: &str) -> Result<String> {
    let mut client = WireClient::spawn(kimi_bin, None, None, None)?;

    let init_params = InitializeParams {
        protocol_version: crate::wire::protocol::KIMI_WIRE_PROTOCOL_VERSION.to_string(),
        client: Some(ClientInfo {
            name: client_name.to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        external_tools: None,
        capabilities: None,
        hooks: None,
    };
    let init_result = client.initialize(init_params).await?;
    info!(
        client = %client_name,
        wire_protocol_version = %init_result.protocol_version,
        "Wire prompt initialized"
    );

    let _prompt_result = client.prompt(prompt).await?;

    let mut text_parts: Vec<String> = Vec::new();

    loop {
        match client.read_message().await {
            Ok(WireMessage::Event(ev)) => {
                match ev.params.to_event() {
                    Ok(Event::TurnEnd) => break,
                    Ok(Event::StepInterrupted) => {
                        anyhow::bail!("Wire prompt was interrupted");
                    }
                    _ => {}
                }

                match ev.params.event_type.as_str() {
                    "turn_end" => break,
                    "step_interrupted" => {
                        anyhow::bail!("Wire prompt was interrupted");
                    }
                    "thinking" | "text" | "content" => {
                        if let Some(text) = ev.params.payload.get("text").and_then(|v| v.as_str()) {
                            text_parts.push(text.to_string());
                        } else if let Some(chunk) =
                            ev.params.payload.get("chunk").and_then(|v| v.as_str())
                        {
                            text_parts.push(chunk.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(WireMessage::Request(req)) => match req.params.to_request() {
                Ok(request) => {
                    let request_type = request.kind();
                    client
                        .send_response(&req.id, request.default_response())
                        .await?;
                    info!(
                        request_id = %req.id,
                        request_type = request_type,
                        client = %client_name,
                        "Handled wire request"
                    );
                }
                Err(_) => {
                    client
                        .send_error(&req.id, -32601, "Request not supported")
                        .await?;
                }
            },
            Ok(WireMessage::SuccessResponse(_)) => {}
            Ok(WireMessage::ErrorResponse(err)) => {
                anyhow::bail!("Wire error response: {:?}", err.error);
            }
            Err(e) => {
                anyhow::bail!("Wire read error: {}", e);
            }
        }
    }

    client.shutdown().await?;
    Ok(text_parts.join(" ").trim().to_string())
}

fn parse_subtasks(text: &str) -> Result<Vec<Subtask>> {
    let trimmed = text.trim();
    if let Ok(tasks) = serde_json::from_str::<Vec<Subtask>>(trimmed) {
        return Ok(tasks);
    }

    let start = trimmed
        .find('[')
        .context("No JSON array found in response")?;
    let end = trimmed
        .rfind(']')
        .context("No JSON array end found in response")?;
    if start >= end {
        anyhow::bail!("Invalid JSON array brackets in response");
    }

    let slice = &trimmed[start..=end];
    let tasks: Vec<Subtask> =
        serde_json::from_str(slice).context("Failed to parse JSON array from response")?;
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_subtasks_accepts_legacy_shape_without_path_sets() {
        let tasks = parse_subtasks(r#"[{"id":"task-1","description":"legacy"}]"#).unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].read_set.is_empty());
        assert!(tasks[0].write_set.is_empty());
    }

    #[test]
    fn parse_subtasks_preserves_read_and_write_sets() {
        let tasks = parse_subtasks(
            r#"Here is the plan:
            [
              {
                "id": "task-1",
                "description": "edit runtime",
                "read_set": ["src/runtime/mod.rs"],
                "write_set": ["src/runtime/scheduler/runner.rs"]
              }
            ]"#,
        )
        .unwrap();

        assert_eq!(tasks[0].read_set, vec!["src/runtime/mod.rs"]);
        assert_eq!(tasks[0].write_set, vec!["src/runtime/scheduler/runner.rs"]);
    }
}
