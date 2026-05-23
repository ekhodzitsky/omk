use std::sync::Arc;

use omk::runtime::classifier::Intent;
use omk::runtime::conversation::bus::{BusEvent, PreflightAction};
use omk::runtime::conversation::outcome::RouteOutcome;
use omk::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    mocks::{MockClassifier, MockGoalBridge, MockLlmDirect, MockWireWorker},
    router::{Router, RouterConfig},
};

use crate::common::{make_classifier_output, make_handle, make_session};

#[tokio::test]
async fn test_large_intent_always_preflights_before_creating_goal() {
    let config = RouterConfig {
        interactive_preflight: true,
        preflight_timeout_ms: 100,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g1")));
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Large,
            0.94,
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
        bridge.clone(),
        config,
        bus.clone(),
    );
    let session = make_session();
    let outcome = router.dispatch("big refactor", &session).await;
    assert!(matches!(outcome, RouteOutcome::Cancelled));

    let mut created = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, BusEvent::ChildGoalCreated { .. }) {
            created = true;
        }
    }
    assert!(!created, "goal should not be created after timeout");
    assert!(bridge.created.lock().await.is_empty());
}

#[tokio::test]
async fn test_large_preflight_accept_creates_child_goal() {
    let config = RouterConfig {
        interactive_preflight: true,
        preflight_timeout_ms: 5_000,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g-large")));
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Large,
            0.94,
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
        bridge.clone(),
        config,
        bus.clone(),
    );
    let session = make_session();
    let router = Arc::new(router);

    let router2 = router.clone();
    let s2 = session.clone();
    let handle = tokio::spawn(async move { router2.dispatch("big refactor", &s2).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            ticket_id = Some(p.ticket_id.clone());
            break;
        }
    }
    assert!(ticket_id.is_some());

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Accept)
        .await;
    let outcome = handle.await.unwrap();
    assert!(matches!(outcome, RouteOutcome::Large { goal_id, .. } if goal_id == "g-large"));
    assert_eq!(bridge.created.lock().await.len(), 1);
}
