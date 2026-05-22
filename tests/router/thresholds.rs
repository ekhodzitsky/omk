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
async fn test_first_prompt_threshold_is_stricter_than_subsequent() {
    let config = RouterConfig {
        interactive_preflight: true,
        first_prompt_threshold: 0.85,
        preflight_timeout_ms: 5_000,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Arc::new(Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Small,
            0.70,
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
    ));
    let session = make_session();

    let router = Arc::new(router);
    let r1 = router.clone();
    let s1 = session.clone();
    let h1 = tokio::spawn(async move { r1.dispatch("edit something", &s1).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            ticket_id = Some(p.ticket_id.clone());
            break;
        }
    }
    assert!(
        ticket_id.is_some(),
        "first prompt with low confidence should preflight"
    );
    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Accept)
        .await;
    let out1 = h1.await.unwrap();
    assert!(matches!(out1, RouteOutcome::Small { .. }));

    let r2 = router.clone();
    let s2 = session.clone();
    let out2 = r2.dispatch("edit something else", &s2).await;
    assert!(matches!(out2, RouteOutcome::Small { .. }));

    let mut second_preflight = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, BusEvent::PreflightRequest(..)) {
            second_preflight = true;
        }
    }
    assert!(!second_preflight);
}
