use omk::runtime::events::{EventKind, EventWriter, RunId};
use omk::runtime::wire_worker::hook_executor::{
    discover_hook_subscriptions, HookExecutor, HookResult,
};
use omk::wire::protocol::HookRequest;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::Duration;

fn make_script(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
    }
    path
}

#[tokio::test]
async fn test_discover_hooks_from_config_toml() {
    let tmp = TempDir::new().unwrap();
    let kimi = tmp.path().join(".kimi");
    let hooks_dir = kimi.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    make_script(&hooks_dir, "safety-check.sh", "#!/bin/bash\nexit 0\n");
    make_script(&hooks_dir, "notify.sh", "#!/bin/bash\nexit 0\n");

    let config = r#"
[[hooks]]
event = "PreToolUse"
command = ".kimi/hooks/safety-check.sh"
matcher = "WriteFile"
timeout = 10

[[hooks]]
event = "SubagentStart"
command = ".kimi/hooks/notify.sh"
timeout = 5
"#;
    std::fs::write(kimi.join("config.toml"), config).unwrap();

    let subs = discover_hook_subscriptions(Some(tmp.path())).await;
    assert_eq!(subs.len(), 2);

    let pre_tool = subs.iter().find(|s| s.event == "PreToolUse").unwrap();
    assert_eq!(pre_tool.id, "pre_tool_use-writefile");
    assert_eq!(pre_tool.matcher.as_deref(), Some("WriteFile"));
    assert_eq!(pre_tool.timeout, Some(10));

    let subagent = subs.iter().find(|s| s.event == "SubagentStart").unwrap();
    assert_eq!(subagent.id, "subagent_start");
    assert_eq!(subagent.matcher, None);
    assert_eq!(subagent.timeout, Some(5));
}

#[tokio::test]
async fn test_discover_hooks_fallback_to_defaults() {
    let tmp = TempDir::new().unwrap();
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Only install one default hook — only that one should be discovered.
    make_script(&hooks_dir, "safety-check.sh", "#!/bin/bash\nexit 0\n");

    let subs = discover_hook_subscriptions(Some(tmp.path())).await;
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].event, "PreToolUse");
}

#[tokio::test]
async fn test_discover_hooks_skips_non_executable() {
    let tmp = TempDir::new().unwrap();
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Write script but do NOT chmod +x
    let path = hooks_dir.join("safety-check.sh");
    std::fs::write(&path, "#!/bin/bash\nexit 0\n").unwrap();

    let subs = discover_hook_subscriptions(Some(tmp.path())).await;
    assert!(subs.is_empty());
}

#[tokio::test]
async fn test_hook_executor_allow_on_exit_zero() {
    let tmp = TempDir::new().unwrap();
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    make_script(
        &hooks_dir,
        "safety-check.sh",
        "#!/bin/bash\necho 'safe'\nexit 0\n",
    );

    let executor = HookExecutor::new(tmp.path());
    let request = HookRequest {
        id: "hook_1".to_string(),
        subscription_id: "pre_tool_use".to_string(),
        event: "PreToolUse".to_string(),
        target: "WriteFile".to_string(),
        input_data: serde_json::json!({"file": "/tmp/test"}),
    };

    let result = executor.run(&request).await.unwrap();
    assert_eq!(result.action, omk::wire::protocol::HookAction::Allow);
    assert_eq!(result.reason, "safe");
}

#[tokio::test]
async fn test_hook_executor_block_on_exit_one() {
    let tmp = TempDir::new().unwrap();
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    make_script(
        &hooks_dir,
        "safety-check.sh",
        "#!/bin/bash\necho 'blocked'\nexit 1\n",
    );

    let executor = HookExecutor::new(tmp.path());
    let request = HookRequest {
        id: "hook_1".to_string(),
        subscription_id: "".to_string(),
        event: "PreToolUse".to_string(),
        target: "WriteFile".to_string(),
        input_data: serde_json::json!({}),
    };

    let result = executor.run(&request).await.unwrap();
    assert_eq!(result.action, omk::wire::protocol::HookAction::Block);
    assert_eq!(result.reason, "blocked");
}

#[tokio::test]
async fn test_hook_executor_block_on_nonzero_exit() {
    let tmp = TempDir::new().unwrap();
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    make_script(
        &hooks_dir,
        "safety-check.sh",
        "#!/bin/bash\necho 'error' >&2\nexit 42\n",
    );

    let executor = HookExecutor::new(tmp.path());
    let request = HookRequest {
        id: "hook_1".to_string(),
        subscription_id: "".to_string(),
        event: "PreToolUse".to_string(),
        target: "WriteFile".to_string(),
        input_data: serde_json::json!({}),
    };

    let result = executor.run(&request).await.unwrap();
    assert_eq!(result.action, omk::wire::protocol::HookAction::Block);
    assert!(result.reason.contains("42"));
    assert!(result.reason.contains("error"));
}

#[tokio::test]
async fn test_hook_executor_timeout() {
    let tmp = TempDir::new().unwrap();
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Script that sleeps longer than the subscription timeout.
    make_script(&hooks_dir, "slow.sh", "#!/bin/bash\nsleep 10\nexit 0\n");

    let config = r#"
[[hooks]]
event = "PreToolUse"
command = ".kimi/hooks/slow.sh"
timeout = 1
"#;
    std::fs::write(tmp.path().join(".kimi").join("config.toml"), config).unwrap();

    let executor = HookExecutor::new(tmp.path());
    let request = HookRequest {
        id: "hook_1".to_string(),
        subscription_id: "".to_string(),
        event: "PreToolUse".to_string(),
        target: "WriteFile".to_string(),
        input_data: serde_json::json!({}),
    };

    let start = tokio::time::Instant::now();
    let result = executor.run(&request).await.unwrap();
    let elapsed = start.elapsed();

    assert_eq!(result.action, omk::wire::protocol::HookAction::Block);
    assert!(result.reason.contains("timed out"));
    assert!(
        elapsed < Duration::from_secs(5),
        "hook should have timed out quickly"
    );
}

#[tokio::test]
async fn test_hook_executor_default_allow_when_no_match() {
    let tmp = TempDir::new().unwrap();
    let executor = HookExecutor::new(tmp.path());
    let request = HookRequest {
        id: "hook_1".to_string(),
        subscription_id: "".to_string(),
        event: "UnknownEvent".to_string(),
        target: "something".to_string(),
        input_data: serde_json::json!({}),
    };

    let result = executor.run(&request).await.unwrap();
    assert_eq!(result.action, omk::wire::protocol::HookAction::Allow);
    assert!(result.reason.contains("No matching hook"));
}

#[test]
fn test_hook_result_response_value() {
    let result = HookResult {
        action: omk::wire::protocol::HookAction::Allow,
        reason: "all good".to_string(),
    };
    let value = result.to_response_value("req-42");
    assert_eq!(value["request_id"], "req-42");
    assert_eq!(value["action"], "allow");
    assert_eq!(value["reason"], "all good");

    let block = HookResult {
        action: omk::wire::protocol::HookAction::Block,
        reason: "nope".to_string(),
    };
    let value = block.to_response_value("req-43");
    assert_eq!(value["action"], "block");
    assert_eq!(value["reason"], "nope");
}

#[tokio::test]
async fn test_hook_event_emission_to_jsonl() {
    let tmp = TempDir::new().unwrap();
    let events_path = tmp.path().join("events.jsonl");
    let writer = EventWriter::new(&events_path);
    let run_id = RunId("run-hook".to_string());

    let triggered = omk::runtime::events::Event::new(run_id.clone(), EventKind::HookTriggered)
        .with_actor("worker-1")
        .with_payload(serde_json::json!({
            "event": "PreToolUse",
            "target": "WriteFile",
            "hook_count": 1,
        }))
        .unwrap();
    writer.append(&triggered).await.unwrap();

    let resolved = omk::runtime::events::Event::new(run_id.clone(), EventKind::HookResolved)
        .with_actor("worker-1")
        .with_payload(serde_json::json!({
            "event": "PreToolUse",
            "target": "WriteFile",
            "action": "allow",
            "reason": "safe",
            "duration_ms": 12,
        }))
        .unwrap();
    writer.append(&resolved).await.unwrap();

    let content = tokio::fs::read_to_string(&events_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["kind"], "hook_triggered");
    assert_eq!(first["actor"], "worker-1");
    assert_eq!(first["payload"]["event"], "PreToolUse");

    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(second["kind"], "hook_resolved");
    assert_eq!(second["payload"]["action"], "allow");
    assert_eq!(second["payload"]["duration_ms"], 12);
}
