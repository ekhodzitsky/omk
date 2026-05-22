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
async fn test_trivial_intent_skips_preflight_and_emits_no_disclosure() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Trivial,
            0.95,
        ))),
        Arc::new(MockLlmDirect::new(42)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "w1".into(),
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
        RouterConfig::default(),
        bus.clone(),
    );
    let session = make_session();
    let outcome = router.dispatch("what is rust", &session).await;
    assert!(matches!(outcome, RouteOutcome::Trivial { .. }));

    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, BusEvent::ClassifierDecided { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(e, BusEvent::RouterEscalating { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(e, BusEvent::DisclosureLine(_))));
}
