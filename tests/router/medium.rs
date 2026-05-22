use std::sync::Arc;

use omk::runtime::classifier::Intent;
use omk::runtime::conversation::bus::BusEvent;
use omk::runtime::conversation::outcome::RouteOutcome;
use omk::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    mocks::{MockClassifier, MockGoalBridge, MockLlmDirect, MockWireWorker},
    router::{Router, RouterConfig},
};

use crate::common::{make_classifier_output, make_handle, make_session};

#[tokio::test]
async fn test_medium_intent_creates_plan_with_n_steps_and_emits_events() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Medium,
            0.88,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "sw1".into(),
                files_touched: 0,
                diff_summary: "".into(),
            },
            MediumPlanResult {
                workers: vec!["mw1".into()],
                steps_completed: 1,
                steps_failed: 0,
            },
        )),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        RouterConfig::default(),
        bus.clone(),
    );
    let session = make_session();
    let outcome = router
        .dispatch("add validation and write tests", &session)
        .await;
    assert!(matches!(outcome, RouteOutcome::Medium { .. }));

    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    assert!(events.iter().any(|e| matches!(
        e,
        BusEvent::RouterEscalating {
            target_mode: omk::runtime::conversation::bus::ActiveMode::PlannerWorkers,
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        BusEvent::WorkerStarted { worker_id, .. } if worker_id == "mw1"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        BusEvent::WorkerCompleted { worker_id, .. } if worker_id == "mw1"
    )));
}
