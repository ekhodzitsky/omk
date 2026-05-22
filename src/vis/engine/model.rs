use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};

use crate::vis::bus::{ActiveMode, EngineEvent, PlanNode, PlanNodeStatus};
pub use crate::vis::engine::blocks::{
    ClassifierBlock, CostBlock, GateBlock, PlanBlock, SessionInfo, SliceBlock, WorkerBlock,
    WorkerStatus,
};
use crate::vis::engine::state::PaneState;

/// Full visual state of the engine pane.
#[derive(Debug, Clone)]
pub struct PaneModel {
    pub session: SessionInfo,
    pub classifier: Option<ClassifierBlock>,
    pub recent_classifications: VecDeque<ClassifierBlock>,
    pub active_mode: ActiveMode,
    pub goal_id: Option<String>,
    pub plan: Option<PlanBlock>,
    pub workers: HashMap<String, WorkerBlock>,
    pub evidence_gates: HashMap<String, GateBlock>,
    pub slices: Vec<SliceBlock>,
    pub cost: CostBlock,
    pub state: PaneState,
    /// Current time as last seen via `SessionTick`.  Used for elapsed-time
    /// calculations so rendering stays deterministic in tests.
    pub now: DateTime<Utc>,
    /// Monotonically-increased on every `SessionTick`; drives spinner frames.
    pub tick_count: usize,
}

impl PaneModel {
    pub fn new(state_machine: super::state::PaneStateMachine) -> Self {
        let now = Utc::now();
        Self {
            session: SessionInfo {
                id: "session".into(),
                project_short: "omk".into(),
                started_at: now,
                uptime: std::time::Duration::ZERO,
            },
            classifier: None,
            recent_classifications: VecDeque::with_capacity(5),
            active_mode: ActiveMode::Idle,
            goal_id: None,
            plan: None,
            workers: HashMap::new(),
            evidence_gates: HashMap::new(),
            slices: Vec::new(),
            cost: CostBlock::default(),
            state: state_machine.state,
            now,
            tick_count: 0,
        }
    }

    /// Current spinner frame (10 frames, cycles on `SessionTick`).
    pub fn spinner_frame(&self) -> char {
        const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[self.tick_count % FRAMES.len()]
    }

    /// Apply a single engine event to the model.
    pub fn apply(&mut self, ev: EngineEvent) {
        match ev {
            EngineEvent::ClassifierDecided {
                intent,
                confidence,
                latency_ms,
                reasoning,
            } => {
                let block = ClassifierBlock {
                    intent,
                    confidence,
                    latency_ms,
                    reasoning: reasoning.clone(),
                    ts: self.now,
                };
                self.classifier = Some(block.clone());
                self.recent_classifications.push_front(block);
                while self.recent_classifications.len() > 5 {
                    self.recent_classifications.pop_back();
                }
            }
            EngineEvent::RouterEscalating { target_mode, .. } => {
                self.active_mode = target_mode;
            }
            EngineEvent::WorkerStarted {
                worker_id,
                kind,
                task,
            } => {
                self.workers.insert(
                    worker_id.clone(),
                    WorkerBlock {
                        worker_id,
                        kind,
                        task,
                        status: WorkerStatus::Running,
                        percent: None,
                        message: None,
                        started_at: self.now,
                    },
                );
            }
            EngineEvent::WorkerProgress {
                worker_id,
                percent,
                message,
            } => {
                if let Some(w) = self.workers.get_mut(&worker_id) {
                    if let Some(p) = percent {
                        w.percent = Some(p);
                    }
                    if let Some(m) = message {
                        w.message = Some(m);
                    }
                }
            }
            EngineEvent::WorkerCompleted { worker_id, ok, .. } => {
                if let Some(w) = self.workers.get_mut(&worker_id) {
                    w.status = if ok {
                        WorkerStatus::Done
                    } else {
                        WorkerStatus::Failed
                    };
                }
            }
            EngineEvent::GoalCreated { goal_id, plan, .. } => {
                self.active_mode = ActiveMode::GoalRun;
                self.goal_id = Some(goal_id.clone());
                self.plan = Some(PlanBlock {
                    goal_id,
                    nodes: plan
                        .into_iter()
                        .enumerate()
                        .map(|(idx, label)| PlanNode {
                            id: format!("p{idx}"),
                            label,
                            status: PlanNodeStatus::Pending,
                        })
                        .collect(),
                    revision: 0,
                });
            }
            EngineEvent::GoalPlanUpdated {
                goal_id,
                revision,
                nodes,
            } => {
                if self.goal_id.as_ref() == Some(&goal_id) {
                    self.plan = Some(PlanBlock {
                        goal_id,
                        nodes,
                        revision,
                    });
                }
            }
            EngineEvent::GoalGateTransition {
                goal_id, gate, to, ..
            } => {
                if self.goal_id.as_ref() == Some(&goal_id) {
                    self.evidence_gates
                        .insert(gate.clone(), GateBlock { gate, state: to });
                }
            }
            EngineEvent::GoalProofReady { goal_id, path: _ } => {
                if self.goal_id.as_ref() == Some(&goal_id) {
                    self.evidence_gates.insert(
                        "proof".into(),
                        GateBlock {
                            gate: "proof".into(),
                            state: "passed".into(),
                        },
                    );
                }
            }
            EngineEvent::SliceOpened {
                goal_id,
                slice_id,
                worktree,
                pr_url,
            } => {
                if self.goal_id.as_ref() == Some(&goal_id) {
                    self.slices.push(SliceBlock {
                        slice_id,
                        worktree,
                        pr_url,
                    });
                }
            }
            EngineEvent::CostDelta {
                tokens_in,
                tokens_out,
                usd,
                ..
            } => {
                self.cost.tokens_in += u64::from(tokens_in);
                self.cost.tokens_out += u64::from(tokens_out);
                self.cost.usd += usd;
            }
            EngineEvent::SessionTick { now } => {
                self.now = now;
                self.tick_count = self.tick_count.wrapping_add(1);
                self.session.uptime = (self.now - self.session.started_at)
                    .to_std()
                    .unwrap_or(std::time::Duration::ZERO);
            }
        }
    }

    /// Number of workers currently in `Running` status.
    pub fn active_worker_count(&self) -> usize {
        self.workers
            .values()
            .filter(|w| w.status == WorkerStatus::Running)
            .count()
    }

    /// Number of workers in `Done` status.
    pub fn completed_worker_count(&self) -> usize {
        self.workers
            .values()
            .filter(|w| w.status == WorkerStatus::Done)
            .count()
    }

    /// Cumulative worker count (all known workers).
    pub fn total_worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Whether any evidence gate is in a failed state.
    pub fn has_failed_gate(&self) -> bool {
        self.evidence_gates.values().any(|g| g.state == "failed")
    }
}

impl Default for PaneModel {
    fn default() -> Self {
        let epoch = DateTime::<Utc>::MIN_UTC;
        Self {
            session: SessionInfo {
                id: "engine".into(),
                project_short: "omk".into(),
                started_at: epoch,
                uptime: std::time::Duration::ZERO,
            },
            classifier: None,
            recent_classifications: VecDeque::with_capacity(5),
            active_mode: ActiveMode::Idle,
            goal_id: None,
            plan: None,
            workers: HashMap::new(),
            evidence_gates: HashMap::new(),
            slices: Vec::new(),
            cost: CostBlock::default(),
            state: PaneState::Collapsed,
            now: epoch,
            tick_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vis::bus::{Intent, PlanNodeStatus};

    #[test]
    fn classifier_decided_updates_recent() {
        let mut model = PaneModel::default();
        for i in 0..7 {
            model.apply(EngineEvent::ClassifierDecided {
                intent: Intent::Trivial,
                confidence: 0.5,
                latency_ms: i,
                reasoning: format!("r{i}"),
            });
        }
        assert_eq!(model.recent_classifications.len(), 5);
        assert_eq!(model.recent_classifications.front().unwrap().latency_ms, 6);
    }

    #[test]
    fn worker_lifecycle() {
        let mut model = PaneModel::default();
        model.apply(EngineEvent::WorkerStarted {
            worker_id: "w1".into(),
            kind: "edit".into(),
            task: "rename".into(),
        });
        assert_eq!(model.workers["w1"].status, WorkerStatus::Running);

        model.apply(EngineEvent::WorkerProgress {
            worker_id: "w1".into(),
            percent: Some(0.5),
            message: Some("half".into()),
        });
        assert_eq!(model.workers["w1"].percent, Some(0.5));

        model.apply(EngineEvent::WorkerCompleted {
            worker_id: "w1".into(),
            files_touched: 3,
            ok: true,
        });
        assert_eq!(model.workers["w1"].status, WorkerStatus::Done);
    }

    #[test]
    fn cost_delta_accumulates() {
        let mut model = PaneModel::default();
        model.apply(EngineEvent::CostDelta {
            source: "a".into(),
            tokens_in: 100,
            tokens_out: 50,
            usd: 0.012,
        });
        model.apply(EngineEvent::CostDelta {
            source: "b".into(),
            tokens_in: 200,
            tokens_out: 100,
            usd: 0.030,
        });
        assert_eq!(model.cost.tokens_in, 300);
        assert_eq!(model.cost.tokens_out, 150);
        assert!((model.cost.usd - 0.042).abs() < 0.0001);
    }

    #[test]
    fn goal_created_sets_plan() {
        let mut model = PaneModel::default();
        model.apply(EngineEvent::GoalCreated {
            goal_id: "g1".into(),
            parent_session: "s1".into(),
            plan: vec!["A".into(), "B".into()],
        });
        assert_eq!(model.active_mode, ActiveMode::GoalRun);
        assert_eq!(model.plan.as_ref().unwrap().nodes.len(), 2);
    }

    #[test]
    fn goal_plan_updated_replaces_nodes() {
        let mut model = PaneModel::default();
        model.apply(EngineEvent::GoalCreated {
            goal_id: "g1".into(),
            parent_session: "s1".into(),
            plan: vec!["A".into(), "B".into()],
        });
        model.apply(EngineEvent::GoalPlanUpdated {
            goal_id: "g1".into(),
            revision: 1,
            nodes: vec![PlanNode {
                id: "p0".into(),
                label: "A".into(),
                status: PlanNodeStatus::Done,
            }],
        });
        assert_eq!(model.plan.as_ref().unwrap().nodes.len(), 1);
        assert_eq!(
            model.plan.as_ref().unwrap().nodes[0].status,
            PlanNodeStatus::Done
        );
    }

    #[test]
    fn session_tick_updates_uptime() {
        let mut model = PaneModel::default();
        let base = DateTime::<Utc>::MIN_UTC + chrono::Duration::seconds(10);
        model.session.started_at = DateTime::<Utc>::MIN_UTC;
        model.apply(EngineEvent::SessionTick { now: base });
        assert_eq!(model.session.uptime.as_secs(), 10);
    }
}
