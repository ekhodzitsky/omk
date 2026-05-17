//! ApprovalProxy — policy engine for autonomous execution of wire worker tasks.
//!
//! Replaces silent auto-approval with configurable policies:
//! - `Never`: always require human approval (default, safe)
//! - `Safe`: auto-approve read-only tools; require approval for mutating tools
//! - `Yolo`: auto-approve everything with explicit structured logging
//! - `Pattern`: auto-approve if tool call matches regex patterns from config

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::wire::protocol::{ApprovalRequest, ApprovalResponseType};

/// Policy modes for the approval proxy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    /// Always require human approval (default).
    #[default]
    Never,
    /// Auto-approve read-only tools; require approval for mutating tools.
    Safe,
    /// Auto-approve everything with explicit structured logging.
    Yolo,
    /// Auto-approve if the action matches any configured regex pattern.
    Pattern { patterns: Vec<String> },
}

impl ApprovalPolicy {
    pub fn parse_name(s: &str) -> Option<Self> {
        match s {
            "never" => Some(ApprovalPolicy::Never),
            "safe" => Some(ApprovalPolicy::Safe),
            "yolo" => Some(ApprovalPolicy::Yolo),
            _ => None,
        }
    }
}

/// The decision an approval proxy can make.
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    Approve,
    ApproveForSession,
    Reject,
}

impl ApprovalDecision {
    pub fn to_response_type(&self) -> ApprovalResponseType {
        match self {
            ApprovalDecision::Approve => ApprovalResponseType::Approve,
            ApprovalDecision::ApproveForSession => ApprovalResponseType::ApproveForSession,
            ApprovalDecision::Reject => ApprovalResponseType::Reject,
        }
    }
}

/// A pending approval awaiting human decision.
#[derive(Debug)]
pub struct PendingApproval {
    pub request: ApprovalRequest,
    pub response_tx: oneshot::Sender<ApprovalDecision>,
}

/// Channel for injecting human decisions into a running wire worker.
#[derive(Debug, Clone)]
pub struct ApprovalChannel {
    pub tx: mpsc::Sender<PendingApproval>,
}

/// Per-worker approval proxy that evaluates `ApprovalRequest`s against a policy.
#[derive(Clone)]
pub struct ApprovalProxy {
    policy: ApprovalPolicy,
    timeout: std::time::Duration,
    channel: Option<ApprovalChannel>,
    /// Pre-compiled regexes for `Pattern` policy to avoid recompilation on every call.
    compiled_patterns: Vec<regex::Regex>,
}

impl std::fmt::Debug for ApprovalProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalProxy")
            .field("policy", &self.policy)
            .field("timeout", &self.timeout)
            .field("channel", &self.channel)
            .field("compiled_patterns", &self.compiled_patterns.len())
            .finish()
    }
}

/// Set of tool names considered read-only for `ApprovalPolicy::Safe`.
fn read_only_tools() -> HashSet<&'static str> {
    [
        "ReadFile",
        "Glob",
        "Grep",
        "SearchWeb",
        "FetchURL",
        "ReadMediaFile",
    ]
    .into_iter()
    .collect()
}

/// Set of tool names considered mutating for `ApprovalPolicy::Safe`.
fn mutating_tools() -> HashSet<&'static str> {
    ["Shell", "WriteFile", "StrReplaceFile", "Agent"]
        .into_iter()
        .collect()
}

impl ApprovalProxy {
    pub fn new(policy: ApprovalPolicy, timeout_secs: u64) -> Self {
        let compiled_patterns = match &policy {
            ApprovalPolicy::Pattern { patterns } => patterns
                .iter()
                .filter_map(|p| regex::Regex::new(p).ok())
                .collect(),
            _ => Vec::new(),
        };
        Self {
            policy,
            timeout: std::time::Duration::from_secs(timeout_secs),
            channel: None,
            compiled_patterns,
        }
    }

    pub fn with_channel(mut self, channel: ApprovalChannel) -> Self {
        self.channel = Some(channel);
        self
    }

    pub fn policy(&self) -> &ApprovalPolicy {
        &self.policy
    }

    /// Evaluate an approval request and return a decision.
    ///
    /// - `Never` → park and wait for human injection (or timeout → reject).
    /// - `Safe`  → auto-approve read-only tools; park mutating tools for human
    ///   approval (or timeout → reject).
    /// - `Yolo`  → auto-approve everything with a structured log line.
    /// - `Pattern` → auto-approve if action matches any pattern; otherwise park.
    pub async fn decide(&self, request: &ApprovalRequest) -> ApprovalDecision {
        match &self.policy {
            ApprovalPolicy::Yolo => {
                info!(
                    request_id = %request.id,
                    action = %request.action,
                    sender = %request.sender,
                    "ApprovalProxy: YOLO auto-approve"
                );
                ApprovalDecision::ApproveForSession
            }
            ApprovalPolicy::Safe => {
                let ro = read_only_tools();
                let mutating = mutating_tools();
                if ro.contains(request.action.as_str()) {
                    info!(
                        request_id = %request.id,
                        action = %request.action,
                        "ApprovalProxy: SAFE auto-approve read-only"
                    );
                    ApprovalDecision::ApproveForSession
                } else if mutating.contains(request.action.as_str()) {
                    info!(
                        request_id = %request.id,
                        action = %request.action,
                        "ApprovalProxy: SAFE mutating tool — awaiting approval"
                    );
                    self.wait_for_human(request).await
                } else {
                    // Unknown tool: conservative path — require approval.
                    warn!(
                        request_id = %request.id,
                        action = %request.action,
                        "ApprovalProxy: SAFE unknown tool — awaiting approval"
                    );
                    self.wait_for_human(request).await
                }
            }
            ApprovalPolicy::Pattern { .. } => {
                if self
                    .compiled_patterns
                    .iter()
                    .any(|re| re.is_match(&request.action))
                {
                    info!(
                        request_id = %request.id,
                        action = %request.action,
                        "ApprovalProxy: PATTERN matched — auto-approve"
                    );
                    ApprovalDecision::ApproveForSession
                } else {
                    self.wait_for_human(request).await
                }
            }
            ApprovalPolicy::Never => {
                info!(
                    request_id = %request.id,
                    action = %request.action,
                    "ApprovalProxy: NEVER policy — awaiting human approval"
                );
                self.wait_for_human(request).await
            }
        }
    }

    /// Park the request and wait for a human decision via the approval channel.
    /// If no channel is configured, immediately reject.
    /// If the timeout fires before a decision arrives, reject.
    async fn wait_for_human(&self, request: &ApprovalRequest) -> ApprovalDecision {
        let Some(ref channel) = self.channel else {
            warn!(
                request_id = %request.id,
                "ApprovalProxy: no human-in-the-loop channel configured — rejecting"
            );
            return ApprovalDecision::Reject;
        };

        let (tx, rx) = oneshot::channel();
        let pending = PendingApproval {
            request: request.clone(),
            response_tx: tx,
        };

        if channel.tx.send(pending).await.is_err() {
            warn!(
                request_id = %request.id,
                "ApprovalProxy: approval channel closed — rejecting"
            );
            return ApprovalDecision::Reject;
        }

        match tokio::time::timeout(self.timeout, rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => {
                warn!(
                    request_id = %request.id,
                    "ApprovalProxy: oneshot receiver dropped — rejecting"
                );
                ApprovalDecision::Reject
            }
            Err(_) => {
                warn!(
                    request_id = %request.id,
                    timeout_secs = %self.timeout.as_secs(),
                    "ApprovalProxy: approval timed out — rejecting"
                );
                ApprovalDecision::Reject
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_request(action: &str) -> ApprovalRequest {
        ApprovalRequest {
            id: "req-1".to_string(),
            tool_call_id: "tc-1".to_string(),
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

    #[tokio::test]
    async fn test_yolo_approves_everything() {
        let proxy = ApprovalProxy::new(ApprovalPolicy::Yolo, 30);
        let req = dummy_request("Shell");
        let decision = proxy.decide(&req).await;
        assert_eq!(decision, ApprovalDecision::ApproveForSession);
    }

    #[tokio::test]
    async fn test_safe_auto_approves_read_only() {
        let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30);
        for action in [
            "ReadFile",
            "Glob",
            "Grep",
            "SearchWeb",
            "FetchURL",
            "ReadMediaFile",
        ] {
            let req = dummy_request(action);
            let decision = proxy.decide(&req).await;
            assert_eq!(
                decision,
                ApprovalDecision::ApproveForSession,
                "expected auto-approve for {action}"
            );
        }
    }

    #[tokio::test]
    async fn test_safe_rejects_mutating_without_channel() {
        let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30);
        let req = dummy_request("Shell");
        let decision = proxy.decide(&req).await;
        assert_eq!(decision, ApprovalDecision::Reject);
    }

    #[tokio::test]
    async fn test_safe_approves_mutating_via_channel() {
        let (tx, mut rx) = mpsc::channel::<PendingApproval>(1);
        let channel = ApprovalChannel { tx };
        let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30).with_channel(channel);

        let req = dummy_request("Shell");
        let proxy_clone = proxy.clone();
        let req_clone = req.clone();

        let decide_handle = tokio::spawn(async move { proxy_clone.decide(&req_clone).await });

        let pending = rx.recv().await.unwrap();
        assert_eq!(pending.request.action, "Shell");
        pending.response_tx.send(ApprovalDecision::Approve).unwrap();

        let decision = decide_handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_never_rejects_without_channel() {
        let proxy = ApprovalProxy::new(ApprovalPolicy::Never, 30);
        let req = dummy_request("ReadFile");
        let decision = proxy.decide(&req).await;
        assert_eq!(decision, ApprovalDecision::Reject);
    }

    #[tokio::test]
    async fn test_pattern_matches_and_approves() {
        let policy = ApprovalPolicy::Pattern {
            patterns: vec![r"^Read.*".to_string(), r"^Glob$".to_string()],
        };
        let proxy = ApprovalProxy::new(policy, 30);
        assert_eq!(
            proxy.decide(&dummy_request("ReadFile")).await,
            ApprovalDecision::ApproveForSession
        );
        assert_eq!(
            proxy.decide(&dummy_request("Glob")).await,
            ApprovalDecision::ApproveForSession
        );
    }

    #[tokio::test]
    async fn test_pattern_rejects_unmatched() {
        let policy = ApprovalPolicy::Pattern {
            patterns: vec![r"^Read.*".to_string()],
        };
        let proxy = ApprovalProxy::new(policy, 30);
        assert_eq!(
            proxy.decide(&dummy_request("Shell")).await,
            ApprovalDecision::Reject
        );
    }

    #[tokio::test]
    async fn test_timeout_rejects_when_human_is_slow() {
        let (tx, mut rx) = mpsc::channel::<PendingApproval>(1);
        let channel = ApprovalChannel { tx };
        let proxy = ApprovalProxy::new(ApprovalPolicy::Never, 1).with_channel(channel);

        let req = dummy_request("Shell");
        let proxy_clone = proxy.clone();
        let req_clone = req.clone();

        let decide_handle = tokio::spawn(async move { proxy_clone.decide(&req_clone).await });

        // Receive but do NOT respond — let it timeout.
        let _pending = rx.recv().await.unwrap();

        let decision = decide_handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Reject);
    }

    #[tokio::test]
    async fn test_unknown_tool_in_safe_mode_requires_approval() {
        let proxy = ApprovalProxy::new(ApprovalPolicy::Safe, 30);
        let req = dummy_request("SomeUnknownTool");
        let decision = proxy.decide(&req).await;
        assert_eq!(decision, ApprovalDecision::Reject);
    }
}
