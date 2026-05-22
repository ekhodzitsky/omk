use std::sync::Arc;
use tokio::sync::broadcast;

pub use crate::runtime::classifier::Intent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMode {
    Idle,
    DirectLlm,
    WireWorker,
    PlannerWorkers,
    GoalRun,
}

#[derive(Debug, Clone)]
pub struct Preflight {
    pub kind: PreflightKind,
    pub headline: String,
    pub timeout_ms: u32,
    pub ticket_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightKind {
    LargeEscalation,
    MediumLowConfidence,
    SmallOverProtectedPath,
    SmallOverManyFiles,
    QueueLargeOnActiveLarge,
    QueueMediumAtConcurrencyCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightAction {
    Accept,
    Explain,
    Downgrade,
    Cancel,
    Timeout,
}

#[derive(Debug, Clone)]
pub enum BusEvent {
    ClassifierDecided {
        intent: Intent,
        confidence: f32,
        latency_ms: u32,
        reasoning: String,
        cached: bool,
        fallback: bool,
    },
    RouterEscalating {
        intent: Intent,
        target_mode: ActiveMode,
        preflight: bool,
    },
    WorkerStarted {
        worker_id: String,
        kind: String,
        task: String,
    },
    WorkerProgress {
        worker_id: String,
        percent: Option<f32>,
        message: Option<String>,
    },
    WorkerCompleted {
        worker_id: String,
        files_touched: u32,
        ok: bool,
    },
    ChildGoalCreated {
        goal_id: String,
        parent_conv_id: String,
        plan: Vec<String>,
    },
    ChildGoalEvent {
        goal_id: String,
        event: crate::runtime::goal::chat_api::ChildGoalEvent,
    },
    CostDelta {
        source: String,
        tokens_in: u32,
        tokens_out: u32,
        usd: f32,
    },
    PreflightRequest(Preflight),
    PreflightResponse(PreflightAction),
    DisclosureLine(String),
    Refused {
        reason: String,
        intent: Intent,
    },
    CostSoftCapWarning {
        current_usd: f32,
        soft_cap_usd: f32,
    },
}

#[derive(Debug)]
pub struct EventBus {
    tx: broadcast::Sender<BusEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BusEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, ev: BusEvent) {
        let _ = self.tx.send(ev);
    }

    pub fn arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
