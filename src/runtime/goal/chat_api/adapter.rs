use crate::runtime::events::{
    Event, EventKind, GateResultPayload, ProofWrittenPayload, RunStartedPayload,
    TaskCompletedPayload, TaskGraphMutationPayload, WorkerStartedPayload,
};

use super::events::{ChildGoalEvent, PlanNode, PlanNodeStatus};

pub fn to_child_event(envelope: &Event) -> Option<ChildGoalEvent> {
    match envelope.kind {
        EventKind::RunStarted => {
            let goal_id = envelope.run_id.0.clone();
            let mut plan = Vec::new();
            if let Some(ref payload) = envelope.payload {
                if let Ok(p) = serde_json::from_value::<RunStartedPayload>(payload.clone()) {
                    plan.push(p.description);
                }
            }
            Some(ChildGoalEvent::Created { goal_id, plan })
        }
        EventKind::TaskGraphMutated => {
            let total_tasks = envelope
                .payload
                .as_ref()
                .and_then(|p| serde_json::from_value::<TaskGraphMutationPayload>(p.clone()).ok())
                .map(|p| p.total_tasks_after)
                .unwrap_or(0);

            let nodes: Vec<PlanNode> = (0..total_tasks)
                .map(|i| PlanNode {
                    id: format!("task-{i}"),
                    label: "Task".to_string(),
                    status: PlanNodeStatus::Pending,
                })
                .collect();

            Some(ChildGoalEvent::PlanUpdated { revision: 0, nodes })
        }
        EventKind::WorkerStarted => {
            let (worker_id, task) = envelope
                .payload
                .as_ref()
                .and_then(|p| serde_json::from_value::<WorkerStartedPayload>(p.clone()).ok())
                .map(|p| (p.worker_id.0, p.role))
                .unwrap_or_else(|| {
                    (
                        envelope.actor.clone().unwrap_or_default(),
                        envelope.actor.clone().unwrap_or_default(),
                    )
                });
            Some(ChildGoalEvent::WorkerStarted { worker_id, task })
        }
        EventKind::TaskOutput => {
            let worker_id = envelope.actor.clone().unwrap_or_default();
            let msg = envelope
                .payload
                .as_ref()
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(ChildGoalEvent::WorkerProgress { worker_id, msg })
        }
        EventKind::TaskCompleted => {
            let worker_id = envelope
                .payload
                .as_ref()
                .and_then(|p| serde_json::from_value::<TaskCompletedPayload>(p.clone()).ok())
                .map(|p| p.worker_id.0)
                .unwrap_or_else(|| envelope.actor.clone().unwrap_or_default());
            Some(ChildGoalEvent::WorkerCompleted {
                worker_id,
                files: 0,
                ok: true,
            })
        }
        EventKind::TaskFailed => {
            let worker_id = envelope.actor.clone().unwrap_or_default();
            Some(ChildGoalEvent::WorkerCompleted {
                worker_id,
                files: 0,
                ok: false,
            })
        }
        EventKind::GatePassed => {
            let gate = envelope
                .payload
                .as_ref()
                .and_then(|p| serde_json::from_value::<GateResultPayload>(p.clone()).ok())
                .map(|p| p.name)
                .unwrap_or_else(|| "unknown".to_string());
            Some(ChildGoalEvent::GateTransition {
                gate,
                from: "unknown".to_string(),
                to: "passed".to_string(),
            })
        }
        EventKind::GateFailed => {
            let gate = envelope
                .payload
                .as_ref()
                .and_then(|p| serde_json::from_value::<GateResultPayload>(p.clone()).ok())
                .map(|p| p.name)
                .unwrap_or_else(|| "unknown".to_string());
            Some(ChildGoalEvent::GateTransition {
                gate,
                from: "unknown".to_string(),
                to: "failed".to_string(),
            })
        }
        EventKind::ProofWritten => {
            let (status, path) = envelope
                .payload
                .as_ref()
                .and_then(|p| serde_json::from_value::<ProofWrittenPayload>(p.clone()).ok())
                .map(|p| (p.status, p.proof_path))
                .unwrap_or_default();
            if status == "ready" {
                Some(ChildGoalEvent::ProofReady { path })
            } else {
                None
            }
        }
        EventKind::RunFailed => {
            let reason = envelope
                .payload
                .as_ref()
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("goal failed")
                .to_string();
            Some(ChildGoalEvent::Failed { reason })
        }
        EventKind::ManualInterrupt => Some(ChildGoalEvent::Cancelled),
        _ => None,
    }
}
