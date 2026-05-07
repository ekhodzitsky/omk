use anyhow::Result;
use tracing::info;

use super::worker::WorkerSpec;

/// Bridge that manages a worker process inside a tmux pane
pub struct TeamBridge<'a> {
    spec: &'a WorkerSpec,
    session: &'a str,
}

impl<'a> TeamBridge<'a> {
    pub fn new(spec: &'a WorkerSpec, session: &'a str) -> Self {
        Self { spec, session }
    }

    /// Spawn a worker bridge in the given tmux pane index
    pub async fn spawn_worker(&self, pane_index: usize) -> Result<()> {
        // The bridge script runs alongside kimi in the same pane.
        // It polls the inbox and launches `kimi -p` for each task.
        let bridge_script = generate_bridge_script(self.spec);
        let script_path = self.spec.inbox.parent().unwrap().join("bridge.sh");
        tokio::fs::write(&script_path, bridge_script).await?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script_path).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script_path, perms).await?;
        }

        let target = format!("{}:.{}", self.session, pane_index);
        let cmd = format!(
            "bash '{}'",
            script_path.display().to_string().replace('\'', "'\"'\"'")
        );

        let output = std::process::Command::new("tmux")
            .args(["send-keys", "-t", &target, &cmd, "Enter"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to spawn bridge: {}", stderr);
        }

        info!(worker = %self.spec.name, pane = pane_index, "Spawned worker bridge");
        Ok(())
    }
}

fn generate_bridge_script(spec: &WorkerSpec) -> String {
    let inbox = spec.inbox.display().to_string();
    let outbox = spec.outbox.display().to_string();
    let heartbeat = spec.heartbeat.display().to_string();
    let name = &spec.name;

    // Use shlex to safely quote paths for bash
    let inbox_q = shlex::try_quote(&inbox).expect("path is valid utf8 without nulls");
    let outbox_q = shlex::try_quote(&outbox).expect("path is valid utf8 without nulls");
    let heartbeat_q = shlex::try_quote(&heartbeat).expect("path is valid utf8 without nulls");
    let name_q = shlex::try_quote(name).expect("name is valid utf8 without nulls");
    let role_q = shlex::try_quote(&spec.role).expect("role is valid utf8 without nulls");

    format!(r#"#!/usr/bin/env bash
set -euo pipefail

INBOX={inbox_q}
OUTBOX={outbox_q}
HEARTBEAT={heartbeat_q}
NAME={name_q}

# Pick JSON parser: jq preferred, fallback to python3
json_get() {{
    local key="$1"
    if command -v jq >/dev/null 2>&1; then
        jq -r ".$key // empty"
    else
        python3 -c "import sys,json; print(json.load(sys.stdin).get('$key',''))"
    fi
}}

# Initialize
mkdir -p "$(dirname "$INBOX")" "$(dirname "$OUTBOX")"
printf '%s\n' '{{"status":"ready","name":"'$NAME'","pid":"'$$'"}}' > "$HEARTBEAT"

echo "[$NAME] Worker bridge ready. Waiting for tasks..."

touch "$INBOX"
LAST_POS=$(wc -c < "$INBOX" | tr -d ' ')

while true; do
    # Update heartbeat
    printf '%s\n' '{{"status":"alive","ts":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'"}}' > "$HEARTBEAT"

    CURRENT_POS=$(wc -c < "$INBOX" | tr -d ' ')
    if [ "$CURRENT_POS" -gt "$LAST_POS" ]; then
        tail -c +$((LAST_POS + 1)) "$INBOX" | while IFS= read -r line; do
            [ -z "$line" ] && continue

            TASK_ID=$(printf '%s\n' "$line" | json_get "id" || echo "unknown")
            TASK_DESC=$(printf '%s\n' "$line" | json_get "task" || echo "$line")

            # Validate task_id is safe (alphanumeric, dash, underscore)
            if ! printf '%s' "$TASK_ID" | grep -qE '^[a-zA-Z0-9_.-]+$'; then
                TASK_ID="invalid-id"
            fi

            echo "[$NAME] Processing task: $TASK_ID"

            # Write task description to temp file to avoid heredoc injection
            TASK_FILE=$(mktemp /tmp/omk-task.XXXXXX)
            printf '%s\n' "$TASK_DESC" > "$TASK_FILE"
            trap 'rm -f "$TASK_FILE"' EXIT

            PROMPT_FILE=$(mktemp /tmp/omk-prompt.XXXXXX)
            cat > "$PROMPT_FILE" <<'PROMPT_EOF'
You are a specialized {role} agent named {name}.
Execute the following task precisely. Report results in structured format.

TASK (ID: TASK_ID_PLACEHOLDER):
PROMPT_EOF
            cat "$TASK_FILE" >> "$PROMPT_FILE"
            cat >> "$PROMPT_FILE" <<'PROMPT_EOF'

RULES:
1. Use tools (ReadFile, Shell, WriteFile) as needed.
2. When complete, output a JSON result block:
   ```result
   {{
     "task_id": "TASK_ID_PLACEHOLDER",
     "status": "success|partial|failed",
     "summary": "one-line summary",
     "artifacts": ["file paths"]
   }}
   ```
3. Be concise but thorough.
PROMPT_EOF

            # Replace placeholder safely
            sed -i.bak "s/TASK_ID_PLACEHOLDER/$TASK_ID/g" "$PROMPT_FILE"
            rm -f "$PROMPT_FILE.bak"

            START_TIME=$(date +%s)
            if command -v kimi >/dev/null 2>&1; then
                OUTPUT=$(kimi -p "$(cat "$PROMPT_FILE")" 2>&1) || true
            else
                OUTPUT="Error: kimi CLI not found"
            fi
            END_TIME=$(date +%s)
            ELAPSED=$((END_TIME - START_TIME))

            rm -f "$PROMPT_FILE" "$TASK_FILE"

            RESULT_JSON=$(printf '%s\n' "$OUTPUT" | awk '/^```result$/,/^```$/{{if (!/^```result$/ && !/^```$/) print}}' | head -1)
            if [ -z "$RESULT_JSON" ]; then
                RESULT_JSON=$(printf '%s\n' "$OUTPUT" | grep -E '^\s*{{' | tail -1)
            fi
            if [ -z "$RESULT_JSON" ]; then
                RESULT_JSON='{{"task_id":"'$TASK_ID'","status":"failed","summary":"No result block found","artifacts":[],"elapsed_secs":'$ELAPSED'}}'
            fi

            printf '%s\n' "$RESULT_JSON" >> "$OUTBOX"
            echo "[$NAME] Completed task: $TASK_ID (${{ELAPSED}}s)"
        done
        LAST_POS=$CURRENT_POS
    fi

    sleep 2
done
"#,
        inbox_q = inbox_q,
        outbox_q = outbox_q,
        heartbeat_q = heartbeat_q,
        name_q = name_q,
        role = role_q,
    )
}
