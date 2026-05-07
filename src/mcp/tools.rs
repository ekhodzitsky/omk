use anyhow::Result;
use serde_json::Value;

pub fn list_tools() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "omk_team",
            "description": "Spawn a team of Kimi agents in tmux",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "description": "Number of workers" },
                    "role": { "type": "string", "description": "Agent role (e.g. coder)" },
                    "task": { "type": "string", "description": "Task description" }
                },
                "required": ["count", "role", "task"]
            }
        }),
        serde_json::json!({
            "name": "omk_status",
            "description": "Check team status",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Team name" }
                },
                "required": ["name"]
            }
        }),
        serde_json::json!({
            "name": "omk_shutdown",
            "description": "Shutdown a team",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Team name" },
                    "force": { "type": "boolean", "description": "Force kill" }
                },
                "required": ["name"]
            }
        }),
    ]
}

pub async fn handle_tool_call(name: &str, arguments: Value) -> Result<Value> {
    match name {
        "omk_team" => {
            let count = arguments["count"].as_u64().unwrap_or(1) as usize;
            let role = arguments["role"].as_str().unwrap_or("coder").to_string();
            let task = arguments["task"].as_str().unwrap_or("").to_string();
            Ok(serde_json::json!({
                "status": "spawned",
                "count": count,
                "role": role,
                "task": task,
                "note": "Team spawn via MCP is not yet fully implemented"
            }))
        }
        "omk_status" => {
            let team_name = arguments["name"].as_str().unwrap_or("").to_string();
            Ok(serde_json::json!({
                "status": "ok",
                "team": team_name,
                "note": "Status check via MCP is not yet fully implemented"
            }))
        }
        "omk_shutdown" => {
            let team_name = arguments["name"].as_str().unwrap_or("").to_string();
            let force = arguments["force"].as_bool().unwrap_or(false);
            Ok(serde_json::json!({
                "status": "shutdown",
                "team": team_name,
                "force": force,
                "note": "Shutdown via MCP is not yet fully implemented"
            }))
        }
        _ => anyhow::bail!("Unknown tool: {}", name),
    }
}
