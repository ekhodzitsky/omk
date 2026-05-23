use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::bus::{BusEvent, Intent};

/// One persisted line in `pending_escalations.jsonl`.
/// Append-only audit trail of every escalation-class event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationLogEntry {
    pub ts: DateTime<Utc>,
    pub kind: EscalationKind,
    pub goal_id: Option<String>,
    pub intent: Option<String>,
    pub summary: String,
    pub confidence: Option<f32>,
    pub auto_proceed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationKind {
    /// Router signalled an escalation (large/preflight kind)
    RouterEscalation,
    /// Worker started for a small/medium task
    WorkerStarted,
    /// Worker completed (ok or fail)
    WorkerCompleted,
    /// Child goal created
    GoalCreated,
    /// Goal gate transitioned (e.g. tests -> running)
    GateTransition,
    /// Proof artefact produced
    ProofReady,
    /// Goal cancelled
    Cancelled,
    /// Goal failed
    Failed,
    /// Refused (e.g. hard cost cap)
    Refused,
}

/// Writer for the session escalation log. Owns a background task that
/// consumes a broadcast::Receiver<BusEvent> and appends to the log
/// file line-atomically.
#[derive(Debug)]
pub struct EscalationLogWriter {
    log_path: PathBuf,
    handle: tokio::task::JoinHandle<()>,
    cancel: CancellationToken,
}

impl EscalationLogWriter {
    /// Spawn a background task that subscribes to `bus_rx` and appends
    /// matching events to `<state_dir>/pending_escalations.jsonl`.
    ///
    /// `state_dir` typically = `~/.local/state/omk/sessions/<id>/`
    ///
    /// Idempotent: the file is created if missing; appends are atomic
    /// per-line (single write syscall with terminating '\n').
    pub fn spawn(state_dir: PathBuf, mut bus_rx: broadcast::Receiver<BusEvent>) -> Result<Self> {
        let log_path = state_dir.join("pending_escalations.jsonl");
        let cancel = CancellationToken::new();
        let ct = cancel.clone();

        let log_path_clone = log_path.clone();
        let handle = tokio::spawn(async move {
            let mut file = match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path_clone)
                .await
            {
                Ok(f) => f,
                Err(e) => {
                    warn!(
                        error = %e,
                        path = %log_path_clone.display(),
                        "EscalationLogWriter failed to open file; dropping all events"
                    );
                    return;
                }
            };

            loop {
                tokio::select! {
                    biased;
                    _ = ct.cancelled() => {
                        debug!(path = %log_path_clone.display(), "EscalationLogWriter received cancellation; draining remaining events");
                        while let Ok(ev) = bus_rx.try_recv() {
                            if let Some(entry) = entry_from_event(&ev) {
                                if let Err(e) = write_entry(&mut file, &entry).await {
                                    warn!(error = %e, "EscalationLogWriter write failed during drain");
                                }
                            }
                        }
                        break;
                    }
                    result = bus_rx.recv() => {
                        match result {
                            Ok(ev) => {
                                if let Some(entry) = entry_from_event(&ev) {
                                    if let Err(e) = write_entry(&mut file, &entry).await {
                                        warn!(error = %e, "EscalationLogWriter write failed");
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!(dropped = n, "escalation log lagged");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }

            debug!(path = %log_path_clone.display(), "EscalationLogWriter shutting down");
        });

        Ok(Self {
            log_path,
            handle,
            cancel,
        })
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Stop the background task; flush any pending writes.
    pub async fn shutdown(self) -> Result<()> {
        self.cancel.cancel();
        self.handle.await.context("escalation log task panicked")?;
        Ok(())
    }
}

async fn write_entry(file: &mut tokio::fs::File, entry: &EscalationLogEntry) -> Result<()> {
    let mut buf = serde_json::to_vec(entry).context("serialize escalation entry")?;
    buf.push(b'\n');
    file.write_all(&buf)
        .await
        .context("write escalation entry")?;
    file.flush().await.context("flush escalation entry")?;
    Ok(())
}

/// Convert a BusEvent into an EscalationLogEntry if it's escalation-class.
/// Returns None for events that are not interesting for the log
/// (e.g. ClassifierDecided trivial, SessionTick — if such exists).
fn entry(
    kind: EscalationKind,
    goal_id: Option<String>,
    intent: Option<String>,
    summary: impl Into<String>,
    auto_proceed: bool,
) -> EscalationLogEntry {
    EscalationLogEntry {
        ts: Utc::now(),
        kind,
        goal_id,
        intent,
        summary: summary.into(),
        confidence: None,
        auto_proceed,
    }
}

pub fn entry_from_event(event: &BusEvent) -> Option<EscalationLogEntry> {
    match event {
        BusEvent::RouterEscalating {
            intent,
            target_mode,
            preflight,
        } => {
            if *intent == Intent::Trivial {
                return None;
            }
            Some(entry(
                EscalationKind::RouterEscalation,
                None,
                Some(format!("{:?}", intent).to_lowercase()),
                format!("escalating to {:?}", target_mode).to_lowercase(),
                !preflight,
            ))
        }
        BusEvent::WorkerStarted {
            worker_id,
            kind,
            task,
        } => Some(entry(
            EscalationKind::WorkerStarted,
            None,
            None,
            format!("{kind}: {task} ({worker_id})"),
            false,
        )),
        BusEvent::WorkerCompleted { worker_id, ok, .. } => Some(entry(
            EscalationKind::WorkerCompleted,
            None,
            None,
            if *ok {
                format!("{worker_id} completed")
            } else {
                format!("{worker_id} failed")
            },
            false,
        )),
        BusEvent::ChildGoalCreated { goal_id, .. } => Some(entry(
            EscalationKind::GoalCreated,
            Some(goal_id.clone()),
            None,
            "child goal created",
            false,
        )),
        BusEvent::ChildGoalEvent { goal_id, event } => match event {
            crate::runtime::goal::chat_api::ChildGoalEvent::GateTransition { gate, from, to } => {
                Some(entry(
                    EscalationKind::GateTransition,
                    Some(goal_id.clone()),
                    None,
                    format!("gate {gate}: {from} -> {to}"),
                    false,
                ))
            }
            crate::runtime::goal::chat_api::ChildGoalEvent::ProofReady { path } => Some(entry(
                EscalationKind::ProofReady,
                Some(goal_id.clone()),
                None,
                format!("proof ready at {}", path.display()),
                false,
            )),
            crate::runtime::goal::chat_api::ChildGoalEvent::Failed { reason } => Some(entry(
                EscalationKind::Failed,
                Some(goal_id.clone()),
                None,
                reason.clone(),
                false,
            )),
            crate::runtime::goal::chat_api::ChildGoalEvent::Cancelled => Some(entry(
                EscalationKind::Cancelled,
                Some(goal_id.clone()),
                None,
                "goal cancelled",
                false,
            )),
            _ => None,
        },
        BusEvent::Refused { reason, intent } => Some(entry(
            EscalationKind::Refused,
            None,
            Some(format!("{:?}", intent).to_lowercase()),
            reason.clone(),
            false,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::classifier::Intent;
    use crate::runtime::conversation::bus::{ActiveMode, BusEvent};
    use crate::runtime::goal::chat_api::ChildGoalEvent;

    #[test]
    fn entry_from_classifier_decided_returns_none() {
        let ev = BusEvent::ClassifierDecided {
            intent: Intent::Small,
            confidence: 0.9,
            latency_ms: 100,
            reasoning: "test".into(),
            cached: false,
            fallback: false,
        };
        assert!(entry_from_event(&ev).is_none());
    }

    #[test]
    fn entry_from_router_escalating_trivial_returns_none() {
        let ev = BusEvent::RouterEscalating {
            intent: Intent::Trivial,
            target_mode: ActiveMode::DirectLlm,
            preflight: false,
        };
        assert!(entry_from_event(&ev).is_none());
    }

    #[test]
    fn entry_from_router_escalating_small_returns_some() {
        let ev = BusEvent::RouterEscalating {
            intent: Intent::Small,
            target_mode: ActiveMode::WireWorker,
            preflight: true,
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::RouterEscalation);
        assert_eq!(entry.intent.as_deref(), Some("small"));
        assert!(!entry.auto_proceed);
    }

    #[test]
    fn entry_from_worker_started_returns_some() {
        let ev = BusEvent::WorkerStarted {
            worker_id: "w1".into(),
            kind: "edit".into(),
            task: "rename".into(),
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::WorkerStarted);
    }

    #[test]
    fn entry_from_worker_completed_ok_returns_some() {
        let ev = BusEvent::WorkerCompleted {
            worker_id: "w1".into(),
            files_touched: 3,
            ok: true,
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::WorkerCompleted);
        assert!(entry.summary.contains("completed"));
    }

    #[test]
    fn entry_from_worker_completed_fail_returns_some() {
        let ev = BusEvent::WorkerCompleted {
            worker_id: "w1".into(),
            files_touched: 0,
            ok: false,
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::WorkerCompleted);
        assert!(entry.summary.contains("failed"));
    }

    #[test]
    fn entry_from_child_goal_event_gate_transition() {
        let ev = BusEvent::ChildGoalEvent {
            goal_id: "g1".into(),
            event: ChildGoalEvent::GateTransition {
                gate: "test".into(),
                from: "running".into(),
                to: "passed".into(),
            },
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::GateTransition);
    }

    #[test]
    fn entry_from_child_goal_event_proof_ready() {
        let ev = BusEvent::ChildGoalEvent {
            goal_id: "g1".into(),
            event: ChildGoalEvent::ProofReady {
                path: std::path::PathBuf::from("/tmp/proof.md"),
            },
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::ProofReady);
    }

    #[test]
    fn entry_from_child_goal_event_cancelled() {
        let ev = BusEvent::ChildGoalEvent {
            goal_id: "g1".into(),
            event: ChildGoalEvent::Cancelled,
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::Cancelled);
    }

    #[test]
    fn entry_from_child_goal_event_failed() {
        let ev = BusEvent::ChildGoalEvent {
            goal_id: "g1".into(),
            event: ChildGoalEvent::Failed {
                reason: "oom".into(),
            },
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::Failed);
        assert_eq!(entry.summary, "oom");
    }

    #[test]
    fn entry_from_refused_returns_some() {
        let ev = BusEvent::Refused {
            reason: "cost cap".into(),
            intent: Intent::Large,
        };
        let entry = entry_from_event(&ev).unwrap();
        assert_eq!(entry.kind, EscalationKind::Refused);
    }

    #[test]
    fn entry_from_cost_delta_returns_none() {
        let ev = BusEvent::CostDelta {
            source: "llm".into(),
            tokens_in: 100,
            tokens_out: 50,
            usd: 0.01,
        };
        assert!(entry_from_event(&ev).is_none());
    }
}
