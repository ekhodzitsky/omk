use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events emitted by the engine runtime and consumed by the visualisation pane.
///
/// This is a lossy projection of the runtime-side `BusEvent` stream (defined in
/// `src/runtime/conversation/bus.rs`).  Tokio-specific handles and other
/// internal details are stripped away — the pane only receives data it can
/// render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    /// Classifier has decided on an intent for the current prompt.
    ClassifierDecided {
        intent: Intent,
        confidence: f32,
        latency_ms: u32,
        reasoning: String,
    },

    /// Router is escalating to a richer execution mode.
    RouterEscalating {
        intent: Intent,
        target_mode: ActiveMode,
        preflight: bool,
    },

    /// A worker process has started.
    WorkerStarted {
        worker_id: String,
        kind: String,
        task: String,
    },

    /// Progress update from an active worker.
    WorkerProgress {
        worker_id: String,
        percent: Option<f32>,
        message: Option<String>,
    },

    /// A worker has finished (successfully or not).
    WorkerCompleted {
        worker_id: String,
        files_touched: u32,
        ok: bool,
    },

    /// A new goal session has been created.
    GoalCreated {
        goal_id: String,
        parent_session: String,
        plan: Vec<String>,
    },

    /// The plan for an existing goal has been updated.
    GoalPlanUpdated {
        goal_id: String,
        revision: u32,
        nodes: Vec<PlanNode>,
    },

    /// An evidence gate changed state.
    GoalGateTransition {
        goal_id: String,
        gate: String,
        from: String,
        to: String,
    },

    /// Proof artifact is ready for inspection.
    GoalProofReady { goal_id: String, path: PathBuf },

    /// A new worktree slice has been opened.
    SliceOpened {
        goal_id: String,
        slice_id: String,
        worktree: PathBuf,
        pr_url: Option<String>,
    },

    /// Incremental cost update.
    CostDelta {
        source: String,
        tokens_in: u32,
        tokens_out: u32,
        usd: f32,
    },

    /// Periodic tick used to update uptime / countdown / idle detection.
    /// Emitted roughly once per second.
    SessionTick { now: DateTime<Utc> },
}

/// Classification bucket for a user prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    Trivial,
    Small,
    Medium,
    Large,
}

/// Active execution mode in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActiveMode {
    #[default]
    Idle,
    DirectLlm,
    WireWorker,
    PlannerWorkers,
    GoalRun,
}

/// A single node in a goal plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: String,
    pub label: String,
    pub status: PlanNodeStatus,
}

/// Status of a plan node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanNodeStatus {
    Pending,
    Running,
    Done,
    Failed,
}

/// Thin wrapper around a `tokio::sync::broadcast` receiver that yields
/// `EngineEvent` values.
#[derive(Debug)]
pub struct EngineSubscriber {
    rx: tokio::sync::broadcast::Receiver<EngineEvent>,
}

impl EngineSubscriber {
    pub fn new(rx: tokio::sync::broadcast::Receiver<EngineEvent>) -> Self {
        Self { rx }
    }

    pub async fn next(&mut self) -> Option<EngineEvent> {
        loop {
            match self.rx.recv().await {
                Ok(ev) => return Some(ev),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_event_roundtrips_via_json() {
        let ev = EngineEvent::ClassifierDecided {
            intent: Intent::Small,
            confidence: 0.92,
            latency_ms: 287,
            reasoning: "rename symbol".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: EngineEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, EngineEvent::ClassifierDecided { .. }));
    }

    #[test]
    fn engine_subscriber_lagged_skips() {
        let (tx, rx) = tokio::sync::broadcast::channel(2);
        tx.send(EngineEvent::SessionTick { now: Utc::now() })
            .unwrap();
        tx.send(EngineEvent::SessionTick { now: Utc::now() })
            .unwrap();
        tx.send(EngineEvent::SessionTick { now: Utc::now() })
            .unwrap();

        let mut sub = EngineSubscriber::new(rx);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let ev = rt.block_on(sub.next());
        assert!(ev.is_some());
    }
}
