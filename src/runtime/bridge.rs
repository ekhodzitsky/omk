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

    format!(r#"#!/usr/bin/env bash
set -euo pipefail

INBOX="{inbox}"
OUTBOX="{outbox}"
HEARTBEAT="{heartbeat}"
NAME="{name}"

# Initialize
mkdir -p "$(dirname "$INBOX")" "$(dirname "$OUTBOX")"
echo '{{"status":"ready","name":"'$NAME'","pid":"$$"}}' > "$HEARTBEAT"

echo "[$NAME] Worker bridge ready. Waiting for tasks..."

touch "$INBOX"
LAST_POS=$(wc -c < "$INBOX" | tr -d ' ')

while true; do
    # Update heartbeat
    echo '{{"status":"alive","ts":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}}' > "$HEARTBEAT"

    # Check for new lines in inbox
    CURRENT_POS=$(wc -c < "$INBOX" | tr -d ' ')
    if [ "$CURRENT_POS" -gt "$LAST_POS" ]; then
        # Read new lines
        tail -c +$((LAST_POS + 1)) "$INBOX" | while IFS= read -r line; do
            if [ -z "$line" ]; then continue; fi
            TASK_ID=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id','unknown'))" 2>/dev/null || echo "unknown")
            TASK_DESC=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('task',''))" 2>/dev/null || echo "$line")

            echo "[$NAME] Processing task: $TASK_ID"

            # Build prompt for kimi
            PROMPT=$(cat <<EOF
You are a specialized {role} agent named {name}.
Execute the following task precisely. Report results in structured format.

TASK (ID: $TASK_ID):
$TASK_DESC

RULES:
1. Use tools (ReadFile, Shell, WriteFile) as needed.
2. When complete, output a JSON result block:
   ```result
   {{
     "task_id": "$TASK_ID",
     "status": "success|partial|failed",
     "summary": "one-line summary",
     "artifacts": ["file paths"]
   }}
   ```
3. Be concise but thorough.
EOF
)

            # Run kimi and capture output
            START_TIME=$(date +%s)
            if command -v kimi >/dev/null 2>&1; then
                OUTPUT=$(kimi -p "$PROMPT" 2>&1) || true
            else
                OUTPUT="Error: kimi CLI not found"
            fi
            END_TIME=$(date +%s)
            ELAPSED=$((END_TIME - START_TIME))

            # Extract result JSON from kimi output
            RESULT_JSON=$(echo "$OUTPUT" | awk '/^```result$/,/^```$/{{if (!/^```result$/ && !/^```$/) print}}' | head -1)
            if [ -z "$RESULT_JSON" ]; then
                # Fallback: try to parse last JSON line
                RESULT_JSON=$(echo "$OUTPUT" | grep -E '^\s*{{' | tail -1)
            fi
            if [ -z "$RESULT_JSON" ]; then
                RESULT_JSON='{{"task_id":"'$TASK_ID'","status":"failed","summary":"No result block found","artifacts":[],"elapsed_secs":'$ELAPSED'}}'
            fi

            # Append to outbox
            echo "$RESULT_JSON" >> "$OUTBOX"
            echo "[$NAME] Completed task: $TASK_ID (${{ELAPSED}}s)"
        done
        LAST_POS=$CURRENT_POS
    fi

    sleep 2
done
"#,
        inbox = inbox,
        outbox = outbox,
        heartbeat = heartbeat,
        name = name,
        role = spec.role,
    )
}
