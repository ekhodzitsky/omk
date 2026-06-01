//! Integration tests for ApprovalProxy and policy engine.

use std::collections::HashSet;

use omk::runtime::events::{EventKind, EventWriter, RunId};
use omk::runtime::wire_worker::{
    ApprovalChannel, ApprovalDecision, ApprovalPolicy, ApprovalProxy, WireWorkerAdapter,
};
use omk::runtime::worker::WorkerSpec;
use omk::wire::ApprovalRequest;
use tokio::sync::mpsc;

fn dummy_approval_request(action: &str) -> ApprovalRequest {
    ApprovalRequest {
        id: "approval_1".to_string(),
        tool_call_id: "call_1".to_string(),
        sender: "agent".to_string(),
        action: action.to_string(),
        description: "do something".to_string(),
        display: None,
        source_kind: None,
        source_id: None,
        agent_id: None,
        subagent_type: None,
        source_description: None,
    }
}

fn make_worker_spec(policy: ApprovalPolicy, timeout_secs: u64) -> WorkerSpec {
    let dir = tempfile::tempdir().unwrap();
    WorkerSpec {
        name: "test-worker".to_string(),
        role: "coder".to_string(),
        inbox: dir.path().join("inbox.jsonl"),
        outbox: dir.path().join("outbox.jsonl"),
        heartbeat: dir.path().join("heartbeat.json"),
        project_dir: None,
        external_tools: None,
        approval_policy: policy,
        approval_timeout_secs: timeout_secs,
    }
}

#[tokio::test]
async fn test_wire_worker_adapter_inherits_approval_policy_from_spec() {
    let spec = make_worker_spec(ApprovalPolicy::Yolo, 60);
    let run_id = RunId("run-1".to_string());
    let events_path = tempfile::tempdir().unwrap().path().join("events.jsonl");
    let event_writer = EventWriter::new(&events_path);

    let adapter = WireWorkerAdapter::new(spec, run_id, event_writer);
    assert_eq!(adapter.approval_policy(), &ApprovalPolicy::Yolo);
}

#[tokio::test]
async fn test_never_policy_rejects_without_human_channel() {
    let proxy = ApprovalProxy::new(ApprovalPolicy::Never, 30);
    let req = dummy_approval_request("Shell");
    let decision = proxy.decide(&req).await;
    assert_eq!(decision, ApprovalDecision::Reject);
}

#[tokio::test]
async fn test_safe_policy_auto_approves_read_only_tools() {
    let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30);
    let read_only = [
        "ReadFile",
        "Glob",
        "Grep",
        "SearchWeb",
        "FetchURL",
        "ReadMediaFile",
    ];

    for action in read_only {
        let req = dummy_approval_request(action);
        let decision = proxy.decide(&req).await;
        assert_eq!(
            decision,
            ApprovalDecision::ApproveForSession,
            "expected auto-approve for {action}"
        );
    }
}

#[tokio::test]
async fn test_safe_policy_rejects_mutating_tools_without_channel() {
    let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30);
    let mutating = ["Shell", "WriteFile", "StrReplaceFile", "Agent"];

    for action in mutating {
        let req = dummy_approval_request(action);
        let decision = proxy.decide(&req).await;
        assert_eq!(
            decision,
            ApprovalDecision::Reject,
            "expected reject for {action} without human channel"
        );
    }
}

#[tokio::test]
async fn test_safe_policy_approves_mutating_tool_via_channel() {
    let (tx, mut rx) = mpsc::channel(1);
    let channel = ApprovalChannel { tx };
    let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30).with_channel(channel);

    let req = dummy_approval_request("Shell");
    let proxy_clone = proxy.clone();
    let req_clone = req.clone();

    let handle = tokio::spawn(async move { proxy_clone.decide(&req_clone).await });

    let pending = rx.recv().await.expect("should receive pending approval");
    assert_eq!(pending.request.action, "Shell");
    pending
        .response_tx
        .send(ApprovalDecision::Approve)
        .expect("should send decision");

    let decision = handle.await.expect("should complete");
    assert_eq!(decision, ApprovalDecision::Approve);
}

#[tokio::test]
async fn test_yolo_policy_approves_everything() {
    let proxy = ApprovalProxy::new(ApprovalPolicy::Yolo, 30);
    let all_tools = [
        "Shell",
        "WriteFile",
        "StrReplaceFile",
        "Agent",
        "ReadFile",
        "Glob",
        "Grep",
        "SearchWeb",
        "FetchURL",
        "ReadMediaFile",
        "UnknownTool",
    ];

    for action in all_tools {
        let req = dummy_approval_request(action);
        let decision = proxy.decide(&req).await;
        assert_eq!(
            decision,
            ApprovalDecision::ApproveForSession,
            "YOLO should approve everything, including {action}"
        );
    }
}

#[tokio::test]
async fn test_pattern_policy_matches_and_approves() {
    let policy = ApprovalPolicy::Pattern {
        patterns: vec![r"^Read.*".to_string(), r"^Glob$".to_string()],
    };
    let proxy = ApprovalProxy::new(policy, 30);

    assert_eq!(
        proxy.decide(&dummy_approval_request("ReadFile")).await,
        ApprovalDecision::ApproveForSession
    );
    assert_eq!(
        proxy.decide(&dummy_approval_request("Glob")).await,
        ApprovalDecision::ApproveForSession
    );
}

#[tokio::test]
async fn test_pattern_policy_rejects_unmatched() {
    let policy = ApprovalPolicy::Pattern {
        patterns: vec![r"^Read.*".to_string()],
    };
    let proxy = ApprovalProxy::new(policy, 30);

    assert_eq!(
        proxy.decide(&dummy_approval_request("Shell")).await,
        ApprovalDecision::Reject
    );
}

#[tokio::test]
async fn test_timeout_rejects_when_human_is_slow() {
    let (tx, mut rx) = mpsc::channel(1);
    let channel = ApprovalChannel { tx };
    let proxy = ApprovalProxy::new(ApprovalPolicy::Never, 1).with_channel(channel);

    let req = dummy_approval_request("Shell");
    let proxy_clone = proxy.clone();
    let req_clone = req.clone();

    let handle = tokio::spawn(async move { proxy_clone.decide(&req_clone).await });

    // Receive but deliberately do NOT respond — force timeout.
    let _pending = rx.recv().await.expect("should receive pending approval");

    let decision = handle.await.expect("should complete");
    assert_eq!(decision, ApprovalDecision::Reject);
}

#[tokio::test]
async fn test_approval_decision_maps_to_response_type() {
    use omk::wire::ApprovalResponseType;

    assert_eq!(
        ApprovalDecision::Approve.to_response_type(),
        ApprovalResponseType::Approve
    );
    assert_eq!(
        ApprovalDecision::ApproveForSession.to_response_type(),
        ApprovalResponseType::ApproveForSession
    );
    assert_eq!(
        ApprovalDecision::Reject.to_response_type(),
        ApprovalResponseType::Reject
    );
}

#[tokio::test]
async fn test_event_kinds_include_approval_variants() {
    let kinds: HashSet<String> = [EventKind::ApprovalRequested, EventKind::ApprovalDecided]
        .iter()
        .map(|k| {
            serde_json::to_string(k)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();

    assert!(kinds.contains("approval_requested"));
    assert!(kinds.contains("approval_decided"));
}

#[tokio::test]
async fn test_config_parses_approval_policy_never() {
    let config_str = r#"
approval_policy = "never"
approval_timeout_secs = 120
"#;
    let config: omk::runtime::config::OmkConfig = toml::from_str(config_str).expect("should parse");
    assert_eq!(config.approval_policy, ApprovalPolicy::Never);
    assert_eq!(config.approval_timeout_secs, 120);
}

#[tokio::test]
async fn test_config_parses_approval_policy_safe() {
    let config_str = r#"
approval_policy = "safe"
"#;
    let config: omk::runtime::config::OmkConfig = toml::from_str(config_str).expect("should parse");
    assert_eq!(config.approval_policy, ApprovalPolicy::Safe);
}

#[tokio::test]
async fn test_config_parses_approval_policy_yolo() {
    let config_str = r#"
approval_policy = "yolo"
"#;
    let config: omk::runtime::config::OmkConfig = toml::from_str(config_str).expect("should parse");
    assert_eq!(config.approval_policy, ApprovalPolicy::Yolo);
}
