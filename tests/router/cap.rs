use std::sync::Arc;
use std::time::{Duration, Instant};

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
async fn test_concurrency_cap_blocks_fourth_medium_goal() {
    let config = RouterConfig {
        interactive_preflight: true,
        medium_goal_cap: 3,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();

    let (tx1, rx1) = tokio::sync::oneshot::channel();
    let (tx2, rx2) = tokio::sync::oneshot::channel();
    let (tx3, rx3) = tokio::sync::oneshot::channel();

    let wire = MockWireWorker::new(
        SmallEditResult {
            worker_id: "sw1".into(),
            files_touched: 0,
            diff_summary: "".into(),
        },
        MediumPlanResult {
            workers: vec!["mw".into()],
            steps_completed: 1,
            steps_failed: 0,
        },
    );
    wire.push_medium_block(rx1).await;
    wire.push_medium_block(rx2).await;
    wire.push_medium_block(rx3).await;

    let router = Arc::new(Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Medium,
            0.88,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(wire),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        config,
        bus.clone(),
    ));

    let session = make_session();

    for _ in 0..3 {
        let r = router.clone();
        let s = session.clone();
        tokio::spawn(async move {
            let _ = r.dispatch("do work", &s).await;
        });
    }

    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if session.active_medium_goals.lock().await.len() == 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(session.active_medium_goals.lock().await.len(), 3);

    let r4 = router.clone();
    let handle4 = tokio::spawn(async move { r4.dispatch("do more work", &session).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            if matches!(p.kind, PreflightKind::QueueMediumAtConcurrencyCap) {
                ticket_id = Some(p.ticket_id.clone());
                break;
            }
        }
    }
    assert!(
        ticket_id.is_some(),
        "expected queue preflight for 4th medium"
    );

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Cancel)
        .await;
    let outcome = handle4.await.unwrap();
    assert!(matches!(outcome, RouteOutcome::Cancelled));

    let _ = tx1.send(());
    let _ = tx2.send(());
    let _ = tx3.send(());
}
