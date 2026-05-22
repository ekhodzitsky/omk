use std::path::PathBuf;
use std::sync::Arc;

use omk::runtime::classifier::Intent;
use omk::runtime::conversation::bus::{BusEvent, PreflightAction, PreflightKind};
use omk::runtime::conversation::outcome::RouteOutcome;
use omk::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    mocks::{MockClassifier, MockGoalBridge, MockLlmDirect, MockWireWorker},
    router::{Router, RouterConfig},
};

use crate::common::{make_classifier_output, make_handle, make_session};

#[tokio::test]
async fn test_small_intent_emits_disclosure_then_spawns_worker() {
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
                files_touched: 2,
                diff_summary: "diff".into(),
            },
            MediumPlanResult {
                workers: vec![],
                steps_completed: 0,
                steps_failed: 0,
            },
        )),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        RouterConfig::default(),
        bus.clone(),
    );
    let session = make_session();
    let outcome = router.dispatch("fix typo", &session).await;
    assert!(matches!(outcome, RouteOutcome::Small { .. }));

    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    let mut iter = events.iter();
    assert!(matches!(
        iter.next(),
        Some(BusEvent::ClassifierDecided { .. })
    ));
    assert!(matches!(
        iter.next(),
        Some(BusEvent::RouterEscalating { .. })
    ));
    assert!(matches!(iter.next(), Some(BusEvent::DisclosureLine(_))));
    assert!(matches!(iter.next(), Some(BusEvent::WorkerStarted { .. })));
    assert!(matches!(
        iter.next(),
        Some(BusEvent::WorkerCompleted { .. })
    ));
    assert!(iter.next().is_none());
}

#[tokio::test]
async fn test_small_with_protected_path_preflights() {
    let config = RouterConfig {
        protected_paths: vec![PathBuf::from(".github/")],
        first_prompt_threshold: 0.0,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Small,
            0.95,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "sw1".into(),
                files_touched: 1,
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
    let router = Arc::new(router);

    let router2 = router.clone();
    let s2 = session.clone();
    let handle =
        tokio::spawn(async move { router2.dispatch("update .github/workflows/x", &s2).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            if matches!(p.kind, PreflightKind::SmallOverProtectedPath) {
                ticket_id = Some(p.ticket_id.clone());
                break;
            }
        }
    }
    assert!(ticket_id.is_some(), "expected preflight for protected path");

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Cancel)
        .await;
    let outcome = handle.await.unwrap();
    assert!(matches!(outcome, RouteOutcome::Cancelled));
}
