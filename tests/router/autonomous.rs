use std::sync::Arc;
use std::time::Duration;

use omk::runtime::classifier::Intent;
use omk::runtime::conversation::bus::{BusEvent, PreflightAction, PreflightKind};
use omk::runtime::conversation::outcome::RouteOutcome;
use omk::runtime::escalation::{
    backends::{MediumPlanResult, SmallEditResult},
    mocks::{MockClassifier, MockGoalBridge, MockLlmDirect, MockWireWorker},
    router::{Router, RouterConfig},
};

use crate::common::{make_classifier_output, make_handle, make_session};

fn default_llm() -> MockLlmDirect {
    MockLlmDirect::new(0)
}

fn default_wire() -> MockWireWorker {
    MockWireWorker::new(
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
    )
}

fn wire_with_workers(workers: Vec<String>) -> MockWireWorker {
    MockWireWorker::new(
        SmallEditResult {
            worker_id: "sw1".into(),
            files_touched: 0,
            diff_summary: "".into(),
        },
        MediumPlanResult {
            workers,
            steps_completed: 1,
            steps_failed: 0,
        },
    )
}

fn make_large_router(
    bus: Arc<omk::runtime::conversation::bus::EventBus>,
    bridge: Arc<MockGoalBridge>,
) -> Router {
    Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Large,
            0.94,
        ))),
        Arc::new(default_llm()),
        Arc::new(default_wire()),
        bridge,
        RouterConfig::default(),
        bus,
    )
}

fn make_low_confidence_router(
    intent: Intent,
    confidence: f32,
    bus: Arc<omk::runtime::conversation::bus::EventBus>,
    bridge: Arc<MockGoalBridge>,
    wire: MockWireWorker,
) -> Router {
    Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            intent, confidence,
        ))),
        Arc::new(default_llm()),
        Arc::new(wire),
        bridge,
        RouterConfig::default(),
        bus,
    )
}

#[tokio::test]
async fn test_autonomous_mode_does_not_arm_preflight_inbox() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g1")));
    let router = make_large_router(bus.clone(), bridge.clone());
    let session = make_session();

    let outcome = router.dispatch("big refactor", &session).await;
    assert!(
        !matches!(outcome, RouteOutcome::Cancelled),
        "autonomous mode should not cancel large escalation"
    );
    assert!(
        !matches!(outcome, RouteOutcome::Refused { .. }),
        "autonomous mode should not refuse large escalation"
    );

    let mut saw_autonomous = false;
    let mut saw_preflight_request = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            BusEvent::AutonomousProceed { .. } => saw_autonomous = true,
            BusEvent::PreflightRequest(..) => saw_preflight_request = true,
            _ => {}
        }
    }
    assert!(saw_autonomous, "expected AutonomousProceed, got none");
    assert!(
        !saw_preflight_request,
        "autonomous mode should not emit PreflightRequest"
    );
}

#[tokio::test]
async fn test_autonomous_proceed_includes_kind_and_intent() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = make_low_confidence_router(
        Intent::Medium,
        0.40,
        bus.clone(),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        wire_with_workers(vec!["mw1".into()]),
    );
    let session = make_session();

    let outcome = router.dispatch("uncertain task", &session).await;
    assert!(
        !matches!(outcome, RouteOutcome::Cancelled),
        "autonomous mode should proceed on low-confidence medium"
    );

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let BusEvent::AutonomousProceed {
            kind,
            intent,
            confidence,
            ..
        } = ev
        {
            assert_eq!(kind, PreflightKind::MediumLowConfidence);
            assert_eq!(intent, Intent::Medium);
            assert!(confidence < 0.65);
            found = true;
        }
    }
    assert!(found, "expected AutonomousProceed with MediumLowConfidence");
}

#[tokio::test]
async fn test_autonomous_mode_still_queues_at_concurrency_cap() {
    let config = RouterConfig {
        medium_goal_cap: 2,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Medium,
            0.88,
        ))),
        Arc::new(default_llm()),
        Arc::new(wire_with_workers(vec!["mw1".into()])),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
        config,
        bus.clone(),
    );
    let session = make_session();
    session
        .active_medium_goals
        .lock()
        .await
        .extend(vec!["plan-a".to_string(), "plan-b".to_string()]);

    let outcome = router.dispatch("do more work", &session).await;
    assert!(
        matches!(outcome, RouteOutcome::Queued { .. }),
        "autonomous mode should queue at cap"
    );

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let BusEvent::AutonomousProceed { kind, .. } = ev {
            assert_eq!(kind, PreflightKind::QueueMediumAtConcurrencyCap);
            found = true;
        }
    }
    assert!(
        found,
        "expected AutonomousProceed for QueueMediumAtConcurrencyCap"
    );
}

#[tokio::test]
async fn test_autonomous_mode_still_queues_active_large() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let router = make_large_router(
        bus.clone(),
        Arc::new(MockGoalBridge::new(make_handle("g1"))),
    );
    let session = make_session();
    session
        .active_large_goal
        .lock()
        .await
        .replace("existing-large".to_string());

    let outcome = router.dispatch("another big refactor", &session).await;
    assert!(
        matches!(outcome, RouteOutcome::Queued { .. }),
        "autonomous mode should queue when large already active"
    );

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let BusEvent::AutonomousProceed { kind, .. } = ev {
            assert_eq!(kind, PreflightKind::QueueLargeOnActiveLarge);
            found = true;
        }
    }
    assert!(
        found,
        "expected AutonomousProceed for QueueLargeOnActiveLarge"
    );
}

#[tokio::test]
async fn test_autonomous_mode_proceeds_without_user_response() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g-auto")));
    let router = Arc::new(make_large_router(bus.clone(), bridge.clone()));
    let session = make_session();

    let r2 = router.clone();
    let s2 = session.clone();
    let handle = tokio::spawn(async move { r2.dispatch("big refactor", &s2).await });

    let outcome = tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .expect("dispatch should complete quickly without waiting for user")
        .unwrap();
    assert!(
        !matches!(outcome, RouteOutcome::Cancelled),
        "autonomous mode should not cancel without user response"
    );
    assert!(
        !matches!(outcome, RouteOutcome::Refused { .. }),
        "autonomous mode should not refuse without user response"
    );
    assert_eq!(bridge.created.lock().await.len(), 1);
}

#[tokio::test]
async fn test_interactive_mode_still_blocks_when_explicitly_enabled() {
    let config = RouterConfig {
        interactive_preflight: true,
        preflight_timeout_ms: 5_000,
        ..Default::default()
    };
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g-interactive")));
    let router = Arc::new(Router::new(
        Arc::new(MockClassifier::new(make_classifier_output(
            Intent::Large,
            0.94,
        ))),
        Arc::new(default_llm()),
        Arc::new(default_wire()),
        bridge.clone(),
        config,
        bus.clone(),
    ));
    let session = make_session();

    let r2 = router.clone();
    let s2 = session.clone();
    let handle = tokio::spawn(async move { r2.dispatch("big refactor", &s2).await });

    let mut ticket_id = None;
    while let Ok(ev) = rx.recv().await {
        match ev {
            BusEvent::PreflightRequest(ref p) => {
                ticket_id = Some(p.ticket_id.clone());
                break;
            }
            BusEvent::AutonomousProceed { .. } => {
                panic!("interactive mode should emit PreflightRequest, not AutonomousProceed");
            }
            _ => {}
        }
    }
    assert!(
        ticket_id.is_some(),
        "expected PreflightRequest in interactive mode"
    );

    router
        .submit_preflight(ticket_id.unwrap(), PreflightAction::Accept)
        .await;
    let outcome = handle.await.unwrap();
    assert!(
        matches!(outcome, RouteOutcome::Large { goal_id, .. } if goal_id == "g-interactive"),
        "interactive mode should create goal after Accept"
    );

    let mut saw_response = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, BusEvent::PreflightResponse(PreflightAction::Accept)) {
            saw_response = true;
        }
    }
    assert!(saw_response, "expected PreflightResponse after Accept");
}

#[tokio::test]
async fn test_autonomous_emits_disclosure_line_for_large_escalations() {
    let bus = omk::runtime::conversation::bus::EventBus::new().arc();
    let mut rx = bus.subscribe();
    let bridge = Arc::new(MockGoalBridge::new(make_handle("g-disclose")));
    let router = make_large_router(bus.clone(), bridge.clone());
    let session = make_session();

    let outcome = router.dispatch("big refactor", &session).await;
    assert!(matches!(outcome, RouteOutcome::Large { .. }));

    let mut saw_autonomous = false;
    let mut saw_disclosure = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            BusEvent::AutonomousProceed { .. } => saw_autonomous = true,
            BusEvent::DisclosureLine(_) => saw_disclosure = true,
            _ => {}
        }
    }
    assert!(
        saw_autonomous,
        "expected AutonomousProceed for large escalation"
    );
    assert!(
        saw_disclosure,
        "expected DisclosureLine in autonomous large escalation"
    );
}
