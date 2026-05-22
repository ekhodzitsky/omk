use std::path::PathBuf;
use std::sync::Arc;

use crate::runtime::conversation::bus::{BusEvent, EventBus};
use crate::runtime::goal::chat_api::ChildGoalEvent;
use crate::vis::bus::{
    ActiveMode as VisActiveMode, EngineEvent, Intent as VisIntent, PlanNode, PlanNodeStatus,
};

/// Subscribe to the runtime `EventBus` and write lossy `EngineEvent`
/// projections into the session `engine-events.jsonl` so the W4 pane
/// can consume them.
///
/// Lifecycle: the spawned task exits automatically when the `EventBus`
/// sender is dropped (closed-channel), so no explicit abort is required.
pub fn start(state_dir: PathBuf, event_bus: Arc<EventBus>) -> tokio::task::JoinHandle<()> {
    let mut rx = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            if let Some(engine_ev) = convert(ev) {
                let val = serde_json::to_value(&engine_ev).unwrap_or_default();
                let _ = crate::runtime::classifier::telemetry::write_engine_event(&state_dir, &val)
                    .await;
            }
        }
    })
}

fn map_intent(i: crate::runtime::classifier::Intent) -> VisIntent {
    match i {
        crate::runtime::classifier::Intent::Trivial => VisIntent::Trivial,
        crate::runtime::classifier::Intent::Small => VisIntent::Small,
        crate::runtime::classifier::Intent::Medium => VisIntent::Medium,
        crate::runtime::classifier::Intent::Large => VisIntent::Large,
    }
}

fn map_active_mode(m: crate::runtime::conversation::bus::ActiveMode) -> VisActiveMode {
    match m {
        crate::runtime::conversation::bus::ActiveMode::Idle => VisActiveMode::Idle,
        crate::runtime::conversation::bus::ActiveMode::DirectLlm => VisActiveMode::DirectLlm,
        crate::runtime::conversation::bus::ActiveMode::WireWorker => VisActiveMode::WireWorker,
        crate::runtime::conversation::bus::ActiveMode::PlannerWorkers => {
            VisActiveMode::PlannerWorkers
        }
        crate::runtime::conversation::bus::ActiveMode::GoalRun => VisActiveMode::GoalRun,
    }
}

fn convert(ev: BusEvent) -> Option<EngineEvent> {
    match ev {
        BusEvent::ClassifierDecided {
            intent,
            confidence,
            latency_ms,
            reasoning,
            ..
        } => Some(EngineEvent::ClassifierDecided {
            intent: map_intent(intent),
            confidence,
            latency_ms,
            reasoning,
        }),
        BusEvent::RouterEscalating {
            intent,
            target_mode,
            preflight,
        } => Some(EngineEvent::RouterEscalating {
            intent: map_intent(intent),
            target_mode: map_active_mode(target_mode),
            preflight,
        }),
        BusEvent::WorkerStarted {
            worker_id,
            kind,
            task,
        } => Some(EngineEvent::WorkerStarted {
            worker_id,
            kind,
            task,
        }),
        BusEvent::WorkerProgress {
            worker_id,
            percent,
            message,
        } => Some(EngineEvent::WorkerProgress {
            worker_id,
            percent,
            message,
        }),
        BusEvent::WorkerCompleted {
            worker_id,
            files_touched,
            ok,
        } => Some(EngineEvent::WorkerCompleted {
            worker_id,
            files_touched,
            ok,
        }),
        BusEvent::ChildGoalCreated {
            goal_id,
            parent_conv_id,
            plan,
        } => Some(EngineEvent::GoalCreated {
            goal_id,
            parent_session: parent_conv_id,
            plan,
        }),
        BusEvent::ChildGoalEvent { goal_id, event } => match event {
            ChildGoalEvent::PlanUpdated { revision, nodes } => {
                let nodes = nodes
                    .into_iter()
                    .map(|n| PlanNode {
                        id: n.id,
                        label: n.label,
                        status: match n.status {
                            crate::runtime::goal::chat_api::PlanNodeStatus::Pending => {
                                PlanNodeStatus::Pending
                            }
                            crate::runtime::goal::chat_api::PlanNodeStatus::Running => {
                                PlanNodeStatus::Running
                            }
                            crate::runtime::goal::chat_api::PlanNodeStatus::Done => {
                                PlanNodeStatus::Done
                            }
                            crate::runtime::goal::chat_api::PlanNodeStatus::Failed => {
                                PlanNodeStatus::Failed
                            }
                        },
                    })
                    .collect();
                Some(EngineEvent::GoalPlanUpdated {
                    goal_id,
                    revision,
                    nodes,
                })
            }
            ChildGoalEvent::GateTransition { gate, from, to } => {
                Some(EngineEvent::GoalGateTransition {
                    goal_id,
                    gate,
                    from,
                    to,
                })
            }
            ChildGoalEvent::SliceOpened {
                slice_id,
                worktree,
                pr_url,
            } => Some(EngineEvent::SliceOpened {
                goal_id,
                slice_id,
                worktree,
                pr_url,
            }),
            ChildGoalEvent::ProofReady { path } => {
                Some(EngineEvent::GoalProofReady { goal_id, path })
            }
            ChildGoalEvent::WorkerStarted { worker_id, task } => Some(EngineEvent::WorkerStarted {
                worker_id,
                kind: "goal".to_string(),
                task,
            }),
            ChildGoalEvent::WorkerCompleted {
                worker_id,
                files,
                ok,
            } => Some(EngineEvent::WorkerCompleted {
                worker_id,
                files_touched: files,
                ok,
            }),
            ChildGoalEvent::WorkerProgress { worker_id, msg } => {
                Some(EngineEvent::WorkerProgress {
                    worker_id,
                    percent: None,
                    message: Some(msg),
                })
            }
            ChildGoalEvent::Failed { .. } | ChildGoalEvent::Cancelled => {
                Some(EngineEvent::WorkerCompleted {
                    worker_id: goal_id,
                    files_touched: 0,
                    ok: false,
                })
            }
            _ => None,
        },
        BusEvent::CostDelta {
            source,
            tokens_in,
            tokens_out,
            usd,
        } => Some(EngineEvent::CostDelta {
            source,
            tokens_in,
            tokens_out,
            usd,
        }),
        _ => None,
    }
}
