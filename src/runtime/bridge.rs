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
            script_path.display().to_string().replace('\'', "'\\''")
        );

        let mut tmux_cmd = tokio::process::Command::new("tmux");
        tmux_cmd.args(["send-keys", "-t", &target, &cmd, "Enter"]);

        let output_str = crate::runtime::retry::retry_command(
            crate::runtime::retry::RetryConfig::default(),
            &mut tmux_cmd,
        )
        .await?;

        info!(worker = %self.spec.name, pane = pane_index, output = %output_str, "Spawned worker bridge");
        Ok(())
    }
}

fn generate_bridge_script(spec: &WorkerSpec) -> String {
    let inbox = spec.inbox.display().to_string();
    let outbox = spec.outbox.display().to_string();
    let heartbeat = spec.heartbeat.display().to_string();
    let name = &spec.name;
    let project_dir = spec
        .project_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    // Use shlex to safely quote paths for bash
    let inbox_q = shlex::try_quote(&inbox).expect("path is valid utf8 without nulls");
    let outbox_q = shlex::try_quote(&outbox).expect("path is valid utf8 without nulls");
    let heartbeat_q = shlex::try_quote(&heartbeat).expect("path is valid utf8 without nulls");
    let name_q = shlex::try_quote(name).expect("name is valid utf8 without nulls");
    let role_q = shlex::try_quote(&spec.role).expect("role is valid utf8 without nulls");
    let project_dir_q = shlex::try_quote(&project_dir).expect("path is valid utf8 without nulls");

    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

INBOX={inbox_q}
OUTBOX={outbox_q}
HEARTBEAT={heartbeat_q}
NAME={name_q}
ROLE={role_q}
PROJECT_DIR={project_dir_q}

# Pick JSON parser: jq preferred, fallback to python3
json_get() {{
    local key="$1"
    if command -v jq >/dev/null 2>&1; then
        jq -r ".$key // empty"
    else
        python3 -c "import sys,json; print(json.load(sys.stdin).get('$key',''))"
    fi
}}

# Build result JSON safely using python3 or jq
json_build_result() {{
    local id="$1"
    local status="$2"
    local result="$3"
    local error="$4"
    local elapsed="$5"
    if command -v python3 >/dev/null 2>&1; then
        python3 -c "
import sys, json
d = {{'id': sys.argv[1], 'status': sys.argv[2], 'result': sys.argv[3], 'error': sys.argv[4], 'elapsed_secs': int(sys.argv[5]) if sys.argv[5] else 0}}
print(json.dumps(d))
" "$id" "$status" "$result" "$error" "$elapsed"
    elif command -v jq >/dev/null 2>&1; then
        jq -n --arg id "$id" --arg status "$status" --arg result "$result" --arg error "$error" --argjson elapsed "${{elapsed:-0}}" '{{"id":$id,"status":$status,"result":$result,"error":$error,"elapsed_secs":$elapsed}}'
    else
        printf '{{"id":"%s","status":"%s","result":"%s","error":"%s","elapsed_secs":%s}}\n' "$id" "$status" "$result" "$error" "${{elapsed:-0}}"
    fi
}}

# Initialize
mkdir -p "$(dirname "$INBOX")" "$(dirname "$OUTBOX")"
printf '%s\n' '{{"status":"ready","name":"'"$NAME"'","pid":"'"$$"'"}}' > "$HEARTBEAT"

echo "[$NAME] Worker bridge ready. Waiting for tasks..."

touch "$INBOX"
LAST_POS=$(wc -c < "$INBOX" | tr -d ' ')
LAST_HB=$(date +%s)

while true; do
    CURRENT_POS=$(wc -c < "$INBOX" | tr -d ' ')

    # Heartbeat every 30 seconds
    NOW=$(date +%s)
    if [ $((NOW - LAST_HB)) -ge 30 ]; then
        printf '%s\n' '{{"status":"alive","ts":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'"}}' > "$HEARTBEAT"
        LAST_HB=$NOW
    fi

    if [ "$CURRENT_POS" -gt "$LAST_POS" ]; then
        tail -c +$((LAST_POS + 1)) "$INBOX" | while IFS= read -r line; do
            [ -z "$line" ] && continue

            TASK_ID=$(printf '%s\n' "$line" | json_get "id" || echo "unknown")
            TASK_DESC=$(printf '%s\n' "$line" | json_get "task" || echo "$line")
            TASK_CRITERIA=$(printf '%s\n' "$line" | json_get "acceptance_criteria" || echo "[]")
            TASK_CONTEXT=$(printf '%s\n' "$line" | json_get "context" || echo "")

            # Validate task_id is safe (alphanumeric, dash, underscore)
            if ! printf '%s' "$TASK_ID" | grep -qE '^[a-zA-Z0-9_.-]+$'; then
                TASK_ID="invalid-id"
            fi

            echo "[$NAME] Processing task: $TASK_ID"

            # Build prompt file safely
            PROMPT_FILE=$(mktemp /tmp/omk-prompt.XXXXXX)
            {{
                printf 'You are a specialized %s agent named %s.\n' "$ROLE" "$NAME"
                printf 'Project directory: %s\n\n' "$PROJECT_DIR"
                printf 'Execute the following task precisely.\n\n'
                printf 'TASK (ID: %s):\n' "$TASK_ID"
                printf '%s\n\n' "$TASK_DESC"
                if [ -n "$TASK_CRITERIA" ] && [ "$TASK_CRITERIA" != "[]" ]; then
                    printf 'Acceptance Criteria:\n%s\n\n' "$TASK_CRITERIA"
                fi
                if [ -n "$TASK_CONTEXT" ]; then
                    printf 'Context:\n%s\n\n' "$TASK_CONTEXT"
                fi
                printf 'When complete, report results clearly.\n'
            }} > "$PROMPT_FILE"

            OUTPUT_FILE=$(mktemp /tmp/omk-output.XXXXXX)
            ERRORS_FILE=$(mktemp /tmp/omk-errors.XXXXXX)

            # Determine kimi binary (support mock for testing)
            KIMI_BIN="${{MOCK_KIMI:-kimi}}"

            START_TIME=$(date +%s)
            if command -v "$KIMI_BIN" >/dev/null 2>&1; then
                if "$KIMI_BIN" -p "$PROMPT_FILE" > "$OUTPUT_FILE" 2> "$ERRORS_FILE"; then
                    ELAPSED=$(( $(date +%s) - START_TIME ))
                    OUTPUT=$(cat "$OUTPUT_FILE")
                    json_build_result "$TASK_ID" "completed" "$OUTPUT" "" "$ELAPSED" >> "$OUTBOX"
                    echo "[$NAME] Completed task: $TASK_ID (${{ELAPSED}}s)"
                else
                    ELAPSED=$(( $(date +%s) - START_TIME ))
                    ERRORS=$(cat "$ERRORS_FILE")
                    json_build_result "$TASK_ID" "failed" "" "$ERRORS" "$ELAPSED" >> "$OUTBOX"
                    echo "[$NAME] Failed task: $TASK_ID (${{ELAPSED}}s)"
                fi
            else
                json_build_result "$TASK_ID" "failed" "" "kimi CLI not found: $KIMI_BIN" "0" >> "$OUTBOX"
                echo "[$NAME] Error: $KIMI_BIN CLI not found"
            fi

            rm -f "$PROMPT_FILE" "$OUTPUT_FILE" "$ERRORS_FILE"
        done
        LAST_POS=$CURRENT_POS
    fi

    sleep 5
done
"#,
        inbox_q = inbox_q,
        outbox_q = outbox_q,
        heartbeat_q = heartbeat_q,
        name_q = name_q,
        role_q = role_q,
        project_dir_q = project_dir_q,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_spec() -> WorkerSpec {
        WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: PathBuf::from("/tmp/w0/inbox.jsonl"),
            outbox: PathBuf::from("/tmp/w0/outbox.jsonl"),
            heartbeat: PathBuf::from("/tmp/w0/heartbeat.json"),
            project_dir: Some(PathBuf::from("/project")),
        }
    }

    #[test]
    fn test_bridge_script_contains_loop_structure() {
        let spec = make_spec();
        let script = generate_bridge_script(&spec);
        assert!(script.contains("while true; do"), "Missing while loop");
        assert!(script.contains("sleep 5"), "Missing sleep 5");
    }

    #[test]
    fn test_bridge_script_uses_kimi_with_prompt_file() {
        let spec = make_spec();
        let script = generate_bridge_script(&spec);
        assert!(
            script.contains("-p \"$PROMPT_FILE\""),
            "Should use kimi -p with prompt file"
        );
    }

    #[test]
    fn test_bridge_script_supports_mock_kimi() {
        let spec = make_spec();
        let script = generate_bridge_script(&spec);
        assert!(
            script.contains("MOCK_KIMI"),
            "Should support MOCK_KIMI env var"
        );
    }

    #[test]
    fn test_bridge_script_includes_project_dir() {
        let spec = make_spec();
        let script = generate_bridge_script(&spec);
        assert!(
            script.contains("Project directory:"),
            "Should include project directory in prompt"
        );
        assert!(
            script.contains("/project"),
            "Should reference actual project dir"
        );
    }

    #[test]
    fn test_bridge_script_heartbeat_every_30s() {
        let spec = make_spec();
        let script = generate_bridge_script(&spec);
        assert!(
            script.contains("LAST_HB"),
            "Should track last heartbeat time"
        );
        assert!(script.contains("-ge 30"), "Should heartbeat every 30s");
    }

    #[test]
    fn test_bridge_script_writes_simple_result_json() {
        let spec = make_spec();
        let script = generate_bridge_script(&spec);
        assert!(
            script.contains("json_build_result"),
            "Should have json_build_result helper"
        );
        assert!(
            script.contains("\"completed\""),
            "Should write completed status"
        );
        assert!(script.contains("\"failed\""), "Should write failed status");
    }

    #[test]
    fn test_bridge_executes_task_with_mock_kimi() {
        let dir = tempfile::tempdir().unwrap();
        let worker_dir = dir.path().join("worker-0");
        std::fs::create_dir_all(&worker_dir).unwrap();

        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
            project_dir: Some(dir.path().to_path_buf()),
        };

        let script = generate_bridge_script(&spec);

        // Write a task before the script starts (with trailing newline for read loop)
        std::fs::write(
            &spec.inbox,
            r#"{"id":"task-1","task":"Say hello"}
"#,
        )
        .unwrap();

        // Modify script to process existing inbox and exit
        let modified = script
            .replace("sleep 5", "break")
            .replace("LAST_POS=$(wc -c < \"$INBOX\" | tr -d ' ')", "LAST_POS=0");

        let mock_kimi = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| {
            let manifest = env!("CARGO_MANIFEST_DIR");
            std::path::PathBuf::from(manifest)
                .join("target")
                .join("debug")
                .join("mock-kimi")
                .to_string_lossy()
                .to_string()
        });
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&modified)
            .env("MOCK_KIMI", mock_kimi)
            .output()
            .expect("failed to run bridge script");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "bridge script failed. stdout: {}\nstderr: {}",
            stdout,
            stderr
        );

        let outbox = std::fs::read_to_string(&spec.outbox).unwrap();
        assert!(
            outbox.contains("task-1"),
            "outbox missing task id: {}",
            outbox
        );
        assert!(
            outbox.contains("completed"),
            "outbox missing completed status: {}",
            outbox
        );
    }

    #[test]
    fn test_bridge_handles_failure_with_mock_kimi() {
        let dir = tempfile::tempdir().unwrap();
        let worker_dir = dir.path().join("worker-0");
        std::fs::create_dir_all(&worker_dir).unwrap();

        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
            project_dir: Some(dir.path().to_path_buf()),
        };

        let script = generate_bridge_script(&spec);

        // Write a task that triggers mock-kimi failure (with trailing newline for read loop)
        std::fs::write(
            &spec.inbox,
            "{\"id\":\"task-fail\",\"task\":\"This will fail\"}\n",
        )
        .unwrap();

        let modified = script
            .replace("sleep 5", "break")
            .replace("LAST_POS=$(wc -c < \"$INBOX\" | tr -d ' ')", "LAST_POS=0");

        let mock_kimi = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| {
            let manifest = env!("CARGO_MANIFEST_DIR");
            std::path::PathBuf::from(manifest)
                .join("target")
                .join("debug")
                .join("mock-kimi")
                .to_string_lossy()
                .to_string()
        });
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&modified)
            .env("MOCK_KIMI", mock_kimi)
            .output()
            .expect("failed to run bridge script");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "bridge script failed. stdout: {}\nstderr: {}",
            stdout,
            stderr
        );

        let outbox = std::fs::read_to_string(&spec.outbox).unwrap();
        assert!(
            outbox.contains("task-fail"),
            "outbox missing task id: {}",
            outbox
        );
        assert!(
            outbox.contains("failed"),
            "outbox missing failed status: {}",
            outbox
        );
    }
}
