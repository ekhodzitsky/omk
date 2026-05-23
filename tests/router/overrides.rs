use std::path::PathBuf;
use std::sync::Arc;

use omk::runtime::classifier::Intent;
use omk::runtime::conversation::bus::{BusEvent, PreflightAction, PreflightKind};
use omk::runtime::conversation::outcome::RouteOutcome;
use omk::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    mocks::{MockClassifier, MockGoalBridge, MockLlmDirect, MockWireWorker},
    overrides::{handle_escalate, handle_quick},
    router::{Router, RouterConfig},
};

use crate::common::{make_classifier_output, make_handle, make_session};

#[tokio::test]
async fn test_quick_override_forces_small_intent_regardless_of_classifier() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g1")));
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Large,
            0.99,
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
        bridge.clone(),
        RouterConfig::default(),
        bus.clone(),
    );
    let session = make_session();
    let outcome = handle_quick(&router, "rewrite auth", &session).await;
    assert!(matches!(outcome, RouteOutcome::Small { .. }));
    assert!(bridge.created.lock().await.is_empty());
}

#[tokio::test]
async fn test_escalate_override_forces_large_with_preflight() {
    let config = RouterConfig {
        interactive_preflight: true,
        preflight_timeout_ms: 5_000,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Arc::new(Router::new(
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
        config,
        bus.clone(),
    ));
    let session = make_session();
    let router = Arc::new(router);

    let r2 = router.clone();
    let s2 = session.clone();
    let h = tokio::spawn(async move { handle_escalate(&r2, "what does foo do", &s2).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            if matches!(p.kind, PreflightKind::LargeEscalation) {
                ticket_id = Some(p.ticket_id.clone());
                break;
            }
        }
    }
    assert!(ticket_id.is_some(), "escalate should always preflight");

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Cancel)
        .await;
    let outcome = h.await.unwrap();
    assert!(matches!(outcome, RouteOutcome::Cancelled));
}

#[tokio::test]
async fn test_preflight_q_downgrades_one_level() {
    let config = RouterConfig {
        interactive_preflight: true,
        preflight_timeout_ms: 5_000,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Arc::new(Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Large,
            0.94,
        ))),
        Arc::new(MockLlmDirect::new(0)),
        Arc::new(MockWireWorker::new(
            SmallEditResult {
                worker_id: "sw1".into(),
                files_touched: 1,
                diff_summary: "".into(),
            },
            MediumPlanResult {
                workers: vec!["mw1".into()],
                steps_completed: 1,
                steps_failed: 0,
            },
        )),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        config,
        bus.clone(),
    ));
    let session = make_session();
    let router = Arc::new(router);

    let r2 = router.clone();
    let s2 = session.clone();
    let h = tokio::spawn(async move { r2.dispatch("big feature", &s2).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            ticket_id = Some(p.ticket_id.clone());
            break;
        }
    }
    assert!(ticket_id.is_some());

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Downgrade)
        .await;
    let outcome = h.await.unwrap();
    assert!(matches!(
        outcome,
        RouteOutcome::Downgraded {
            from: Intent::Large,
            to: Intent::Medium,
            ..
        }
    ));
}

#[tokio::test]
async fn test_preflight_q_on_small_is_noop_log() {
    let config = RouterConfig {
        interactive_preflight: true,
        protected_paths: vec![PathBuf::from(".github/")],
        first_prompt_threshold: 0.0,
        preflight_timeout_ms: 5_000,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Arc::new(Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Small,
            0.95,
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
    ));
    let session = make_session();
    let router = Arc::new(router);

    let r2 = router.clone();
    let s2 = session.clone();
    let h = tokio::spawn(async move {
        r2.dispatch_with_intent_override("change .github/x", Intent::Small, &s2)
            .await
    });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        if let BusEvent::PreflightRequest(ref p) = ev {
            if matches!(p.kind, PreflightKind::SmallOverProtectedPath) {
                ticket_id = Some(p.ticket_id.clone());
                break;
            }
        }
    }
    assert!(ticket_id.is_some());

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Downgrade)
        .await;
    let outcome = h.await.unwrap();
    assert!(
        matches!(outcome, RouteOutcome::Refused { reason } if reason.contains("already lowest non-trivial"))
    );
}
