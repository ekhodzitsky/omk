use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ChildGoalEvent {
    Created {
        goal_id: String,
        plan: Vec<String>,
    },
    PlanUpdated {
        revision: u32,
        nodes: Vec<PlanNode>,
    },
    WorkerStarted {
        worker_id: String,
        task: String,
    },
    WorkerProgress {
        worker_id: String,
        msg: String,
    },
    WorkerCompleted {
        worker_id: String,
        files: u32,
        ok: bool,
    },
    GateTransition {
        gate: String,
        from: String,
        to: String,
    },
    SliceOpened {
        slice_id: String,
        worktree: PathBuf,
        pr_url: Option<String>,
    },
    ProofReady {
        path: PathBuf,
    },
    Failed {
        reason: String,
    },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct PlanNode {
    pub id: String,
    pub label: String,
    pub status: PlanNodeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanNodeStatus {
    Pending,
    Running,
    Done,
    Failed,
}
