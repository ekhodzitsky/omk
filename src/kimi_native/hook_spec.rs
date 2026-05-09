use serde::{Deserialize, Serialize};

/// Kimi CLI hook event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    Stop,
    StopFailure,
    SessionStart,
    SessionEnd,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    Notification,
}

/// A single hook definition for Kimi config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub event: HookEvent,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// All hooks OMK recommends for a project.
#[derive(Debug, Clone, Default)]
pub struct ProjectHookDefs {
    pub hooks: Vec<HookConfig>,
    pub scripts: Vec<(String, String)>, // (filename, content)
}

pub fn default_project_hooks() -> ProjectHookDefs {
    let mut defs = ProjectHookDefs::default();

    // Safety check: block edits to sensitive files
    defs.hooks.push(HookConfig {
        event: HookEvent::PreToolUse,
        command: ".kimi/hooks/safety-check.sh".to_string(),
        matcher: Some("WriteFile|StrReplaceFile".to_string()),
        timeout: Some(10),
    });
    defs.scripts.push((
        "safety-check.sh".to_string(),
        r#"#!/bin/bash
# OMK Safety Hook — blocks edits to sensitive files
# Receives JSON via stdin

set -e
INPUT=$(cat)
FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

BLOCKED="\.env$ \.env\.local$ id_rsa id_dsa \.p12$ \.key$"
for pattern in $BLOCKED; do
    if echo "$FILE" | grep -Eq "$pattern"; then
        echo "🛡️  OMK safety hook: blocking edit to sensitive file: $FILE" >&2
        exit 2
    fi
done

exit 0
"#
        .to_string(),
    ));

    // Completion check: verify gates before stop
    defs.hooks.push(HookConfig {
        event: HookEvent::Stop,
        command: ".kimi/hooks/completion-check.sh".to_string(),
        matcher: None,
        timeout: Some(60),
    });
    defs.scripts.push((
        "completion-check.sh".to_string(),
        r#"#!/bin/bash
# OMK Completion Hook — verify gates before Kimi considers a turn complete
# Receives JSON via stdin

set -e
INPUT=$(cat)
CWD=$(echo "$INPUT" | jq -r '.cwd // empty')

# Run verification gates if .omk/gates.toml exists
if [ -f "$CWD/.omk/gates.toml" ]; then
    echo "🔍 OMK completion hook: running verification gates..."
    # Gates are managed by OMK runtime, this hook just logs for now
fi

exit 0
"#
        .to_string(),
    ));

    // Notification hook: log subagent lifecycle
    defs.hooks.push(HookConfig {
        event: HookEvent::SubagentStart,
        command: ".kimi/hooks/notify.sh".to_string(),
        matcher: None,
        timeout: Some(5),
    });
    defs.hooks.push(HookConfig {
        event: HookEvent::SubagentStop,
        command: ".kimi/hooks/notify.sh".to_string(),
        matcher: None,
        timeout: Some(5),
    });
    defs.scripts.push((
        "notify.sh".to_string(),
        r#"#!/bin/bash
# OMK Notification Hook — logs subagent lifecycle events
# Receives JSON via stdin

set -e
INPUT=$(cat)
EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty')
AGENT=$(echo "$INPUT" | jq -r '.agent_name // empty')

# Append to OMK event log
LOG_DIR="${OMK_STATE_DIR:-$HOME/.omk/state}/events"
mkdir -p "$LOG_DIR"
echo "$(date -Iseconds) $EVENT agent=$AGENT" >> "$LOG_DIR/kimi-hooks.log"

exit 0
"#
        .to_string(),
    ));

    defs
}
