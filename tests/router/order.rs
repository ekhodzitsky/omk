use std::sync::Arc;

use omk::runtime::classifier::Intent;
use omk::runtime::conversation::bus::BusEvent;
use omk::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    mocks::{MockClassifier, MockGoalBridge, MockLlmDirect, MockWireWorker},
    router::{Router, RouterConfig},
};

use crate::common::{make_classifier_output, make_handle, make_session};

#[tokio::test]
async fn test_disclosure_line_emitted_before_any_side_effect() {
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
    let _ = router.dispatch("fix", &session).await;

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
    assert!(matches!(iter.next(), Some(BusEvent::DisclosureLine { .. })));
    assert!(matches!(iter.next(), Some(BusEvent::WorkerStarted { .. })));
}
