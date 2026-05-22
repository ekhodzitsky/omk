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
async fn test_soft_cost_cap_warns_once_per_session() {
    let config = RouterConfig {
        cost_cap_usd_soft: Some(1.0),
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Small,
            0.92,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "sw1".into(),
                files_touched: 0,
                diff_summary: "".into(),
            },
            MediumPlanResult {
                workers: vec![],
                steps_completed: 0,
                steps_failed: 0,
            },
        )),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        config,
        bus.clone(),
    );
    let session = make_session();
    *session.cumulative_cost_usd.lock().await = 1.5;

    let _ = router.dispatch("edit", &session).await;

    let mut warned = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, BusEvent::CostSoftCapWarning { .. }) {
            warned = true;
        }
    }
    assert!(warned, "soft cap warning should be emitted");

    let mut rx2 = bus.subscribe();
    let _ = router.dispatch("edit again", &session).await;
    let mut warned2 = false;
    while let Ok(ev) = rx2.try_recv() {
        if matches!(ev, BusEvent::CostSoftCapWarning { .. }) {
            warned2 = true;
        }
    }
    assert!(!warned2, "soft cap warning should not repeat");
}

#[tokio::test]
async fn test_hard_cost_cap_refuses_new_dispatches() {
    let config = RouterConfig {
        cost_cap_usd_hard: Some(5.0),
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Small,
            0.92,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "sw1".into(),
                files_touched: 0,
                diff_summary: "".into(),
            },
            MediumPlanResult {
                workers: vec![],
                steps_completed: 0,
                steps_failed: 0,
            },
        )),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        config,
        bus.clone(),
    );
    let session = make_session();
    *session.cumulative_cost_usd.lock().await = 5.5;

    let outcome_small = router.dispatch("edit", &session).await;
    assert!(
        matches!(outcome_small, RouteOutcome::Refused { reason } if reason.contains("hard cost cap exceeded"))
    );

    let router2 = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Trivial,
            0.99,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "sw1".into(),
                files_touched: 0,
                diff_summary: "".into(),
            },
            MediumPlanResult {
                workers: vec![],
                steps_completed: 0,
                steps_failed: 0,
            },
        )),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        RouterConfig {
            cost_cap_usd_hard: Some(5.0),
            ..Default::default()
        },
        bus.clone(),
    );
    let outcome_trivial = router2.dispatch("hello", &session).await;
    assert!(matches!(outcome_trivial, RouteOutcome::Trivial { .. }));
}
