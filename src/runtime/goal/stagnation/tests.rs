use chrono::Utc;

use crate::runtime::gates::GateResult;
use crate::runtime::goal::budget::GoalBudgetCheckpoint;
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::stagnation::checkpoint::{list_checkpoints, RecoveryCheckpoint};
use crate::runtime::goal::stagnation::collector::{IterationMetrics, StagnationCollector};
use crate::runtime::goal::stagnation::detector::StagnationDetector;
use crate::runtime::goal::stagnation::diagnosis::{DiagnosisEngine, StagnationCause};
use crate::runtime::goal::stagnation::recovery::{RecoveryPlanner, RecoveryStrategy, RiskLevel};
use crate::runtime::goal::state::{GoalPhase, GoalStatus};
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};

fn make_flat_metrics(iteration: u32) -> IterationMetrics {
    IterationMetrics {
        iteration,
        proof_score: 0.3,
        commit_velocity: 0,
        gate_pass_rate: 0.3,
        coverage_delta: None,
        tokens_spent: 5000,
        files_touched: 3,
        timestamp: Utc::now(),
    }
}

fn make_improving_metrics(iteration: u32) -> IterationMetrics {
    IterationMetrics {
        iteration,
        proof_score: 0.1 * iteration as f64,
        commit_velocity: iteration,
        gate_pass_rate: 0.1 * iteration as f64,
        coverage_delta: Some(0.5),
        tokens_spent: 1000,
        files_touched: 2,
        timestamp: Utc::now(),
    }
}

fn make_gate(name: &str, passed: bool, stderr: &str) -> GateResult {
    GateResult {
        name: name.to_string(),
        passed,
        stdout: String::new(),
        stderr: stderr.to_string(),
        duration_ms: 100,
        required: true,
        command_line: String::new(),
        exit_code: if passed { Some(0) } else { Some(1) },
        timed_out: false,
        stdout_summary: None,
        stderr_summary: None,
        output_path: None,
        timeout_secs: 0,
    }
}

fn make_budget(used_tokens: u64) -> GoalBudgetCheckpoint {
    GoalBudgetCheckpoint {
        version: 1,
        goal_id: "test-goal".to_string(),
        label: "test".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        recorded_at: Utc::now(),
        budget_time: None,
        total_budget_secs: None,
        elapsed_since_created_secs: 0,
        remaining_budget_secs: None,
        budget_tokens: None,
        used_tokens,
        remaining_budget_tokens: None,
        budget_usd: None,
        estimated_cost_usd: 0.0,
        remaining_budget_usd: None,
    }
}

fn make_proof(score_hint: f64) -> GoalProof {
    GoalProof {
        version: 1,
        goal_id: "test-goal".to_string(),
        status: GoalStatus::Running,
        readiness: "test".to_string(),
        summary: "test".to_string(),
        generated_at: Utc::now(),
        artifacts: Vec::new(),
        task_graph_summary: crate::runtime::goal::task_graph::GoalTaskGraphSummary {
            total_tasks: 1,
            done_tasks: 0,
            pending_tasks: 1,
            blocked_tasks: 0,
        },
        changed_files: Vec::new(),
        commits: vec!["abc".to_string()],
        git: None,
        gates: if score_hint > 0.0 {
            vec![make_gate("test", score_hint >= 0.5, "")]
        } else {
            Vec::new()
        },
        post_mutation_gates_ran: false,
        known_gaps: Vec::new(),
        human_decisions_required: Vec::new(),
        recovery_status: None,
    }
}

#[test]
fn stagnation_detected_for_flat_iterations() {
    let detector = StagnationDetector::default();
    // Need warmup (3) + window (5) = 8 iterations minimum for detection
    let history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    let report = detector
        .detect(&history, GoalStatus::Running, GoalPhase::Execution)
        .expect("stagnation should be detected");
    assert!(report.stagnant_metrics.len() >= 3);
}

#[test]
fn no_stagnation_for_improving_iterations() {
    let detector = StagnationDetector::default();
    // Need warmup (3) + window (5) = 8 iterations minimum for detection
    let history: Vec<_> = (1..=8).map(make_improving_metrics).collect();
    let report = detector.detect(&history, GoalStatus::Running, GoalPhase::Execution);
    assert!(report.is_none(), "improving metrics should not be stagnant");
}

#[test]
fn no_stagnation_during_warmup() {
    let detector = StagnationDetector::default();
    let history: Vec<_> = (1..=7).map(make_flat_metrics).collect();
    let report = detector.detect(&history, GoalStatus::Running, GoalPhase::Execution);
    assert!(report.is_none(), "warmup should prevent detection");
}

#[test]
fn no_stagnation_when_proof_complete() {
    let detector = StagnationDetector::default();
    let mut history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    history.last_mut().unwrap().proof_score = 1.0;
    let report = detector.detect(&history, GoalStatus::Running, GoalPhase::Execution);
    assert!(report.is_none(), "complete proof should not be stagnant");
}

#[test]
fn no_stagnation_when_proof_near_complete() {
    let detector = StagnationDetector::default();
    let mut history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    history.last_mut().unwrap().proof_score = 0.999;
    let report = detector.detect(&history, GoalStatus::Running, GoalPhase::Execution);
    assert!(
        report.is_none(),
        "near-complete proof (within epsilon) should not be stagnant"
    );
}

#[test]
fn no_stagnation_for_oscillating_proof_score() {
    let detector = StagnationDetector::default();
    let mut history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    // Oscillate wildly: first == last but middle varies
    for (i, m) in history.iter_mut().enumerate() {
        m.proof_score = if i % 2 == 0 { 0.0 } else { 0.5 };
    }
    let report = detector.detect(&history, GoalStatus::Running, GoalPhase::Execution);
    assert!(
        report.is_none(),
        "oscillating proof score should not be flagged stagnant"
    );
}

#[test]
fn coverage_none_does_not_count_as_stagnant() {
    let detector = StagnationDetector::default();
    let history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    let report = detector
        .detect(&history, GoalStatus::Running, GoalPhase::Execution)
        .expect("stagnation should be detected on other metrics");
    assert!(
        !report
            .stagnant_metrics
            .contains(&"coverage_delta".to_string()),
        "missing coverage data should not be counted as stagnant"
    );
}

#[test]
fn diagnosis_detects_test_flakiness() {
    let engine = DiagnosisEngine::new(0.3);
    let history: Vec<_> = (1..=5).map(make_flat_metrics).collect();

    let gates_history: Vec<Vec<GateResult>> = vec![
        vec![make_gate("test", true, "")],
        vec![make_gate("test", false, "connection refused")],
        vec![make_gate("test", true, "")],
        vec![make_gate("test", false, "connection refused")],
        vec![make_gate("test", true, "")],
    ];

    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_eq!(report.cause, StagnationCause::TestFlakiness);
    assert!(
        report.confidence > 0.5,
        "confidence should be high for clear flakiness"
    );
    assert!(!report.evidence.is_empty());
}

#[test]
fn diagnosis_no_flakiness_when_gates_stable() {
    let engine = DiagnosisEngine::new(0.3);
    let history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    let gates_history: Vec<Vec<GateResult>> =
        (0..5).map(|_| vec![make_gate("test", true, "")]).collect();
    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_ne!(
        report.cause,
        StagnationCause::TestFlakiness,
        "stable gates should not trigger test flakiness"
    );
}

#[test]
fn diagnosis_detects_scope_too_large() {
    let engine = DiagnosisEngine::new(0.3);
    let mut history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    for m in &mut history {
        m.proof_score = 0.3;
        m.files_touched = 15;
    }

    let gates_history: Vec<Vec<GateResult>> =
        (0..5).map(|_| vec![make_gate("test", false, "")]).collect();
    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_eq!(report.cause, StagnationCause::ScopeTooLarge);
    assert!(report.confidence > 0.0);
}

#[test]
fn diagnosis_detects_external_dependency_broken() {
    let engine = DiagnosisEngine::new(0.3);
    let history: Vec<_> = (1..=5).map(make_flat_metrics).collect();

    let err = "connection refused to db:5432";
    let gates_history: Vec<Vec<GateResult>> = vec![
        vec![make_gate("test", false, err)],
        vec![make_gate("test", false, err)],
        vec![make_gate("test", false, err)],
        vec![make_gate("test", false, err)],
        vec![make_gate("test", false, err)],
    ];

    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_eq!(report.cause, StagnationCause::ExternalDependencyBroken);
    assert!(report.confidence > 0.5);
    assert!(!report.affected_gates.is_empty());
}

#[test]
fn diagnosis_no_external_dependency_when_stderr_differs() {
    let engine = DiagnosisEngine::new(0.3);
    let history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    let gates_history: Vec<Vec<GateResult>> = vec![
        vec![make_gate("test", false, "error A")],
        vec![make_gate("test", false, "error B")],
        vec![make_gate("test", false, "error C")],
        vec![make_gate("test", false, "error D")],
        vec![make_gate("test", false, "error E")],
    ];
    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_ne!(
        report.cause,
        StagnationCause::ExternalDependencyBroken,
        "diverse stderr should not trigger external dependency"
    );
}

#[test]
fn diagnosis_detects_circular_fix() {
    let engine = DiagnosisEngine::new(0.3);
    let history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    let gates_history: Vec<Vec<GateResult>> =
        (0..5).map(|_| vec![make_gate("test", true, "")]).collect();

    let changed_files_history: Vec<Vec<String>> = vec![
        vec!["src/lib.rs".to_string()],
        vec![],
        vec!["src/lib.rs".to_string()],
        vec![],
        vec!["src/lib.rs".to_string()],
    ];

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_eq!(report.cause, StagnationCause::CircularFix);
    assert!(report.confidence > 0.0);
    assert!(!report.affected_files.is_empty());
}

#[test]
fn diagnosis_no_circular_fix_when_files_stable() {
    let engine = DiagnosisEngine::new(0.3);
    let history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    let gates_history: Vec<Vec<GateResult>> =
        (0..5).map(|_| vec![make_gate("test", true, "")]).collect();
    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_ne!(
        report.cause,
        StagnationCause::CircularFix,
        "stable files should not trigger circular fix"
    );
}

#[test]
fn diagnosis_detects_inefficient_exploration() {
    let engine = DiagnosisEngine::new(0.3);
    let mut history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    for m in &mut history {
        m.tokens_spent = 50000;
        m.proof_score = 0.3;
    }
    let gates_history: Vec<Vec<GateResult>> =
        (0..5).map(|_| vec![make_gate("test", true, "")]).collect();
    let changed_files_history: Vec<Vec<String>> =
        (0..5).map(|_| vec!["src/lib.rs".to_string()]).collect();

    let report = engine.diagnose(&history, &gates_history, &changed_files_history);
    assert_eq!(report.cause, StagnationCause::InefficientExploration);
    assert!(report.confidence > 0.0);
}

#[test]
fn recovery_plan_for_test_flakiness() {
    let planner = RecoveryPlanner::new();
    let diagnosis = DiagnosisEngine::new(0.3).diagnose(
        &(1..=5).map(make_flat_metrics).collect::<Vec<_>>(),
        &[
            vec![make_gate("test", true, "")],
            vec![make_gate("test", false, "err")],
            vec![make_gate("test", true, "")],
        ],
        &[
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
        ],
    );
    let plan = planner.plan(&diagnosis);
    assert_eq!(plan.cause, StagnationCause::TestFlakiness);
    assert_eq!(plan.strategy, RecoveryStrategy::MockExternalDeps);
    assert_eq!(plan.risk_level, RiskLevel::Low);
    assert!(plan.estimated_tokens.is_some());
}

#[test]
fn recovery_plan_for_scope_too_large() {
    let planner = RecoveryPlanner::new();
    let mut history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    for m in &mut history {
        m.files_touched = 15;
        m.proof_score = 0.3;
    }
    let diagnosis = DiagnosisEngine::new(0.3).diagnose(
        &history,
        &[
            vec![make_gate("test", false, "")],
            vec![make_gate("test", false, "")],
            vec![make_gate("test", false, "")],
        ],
        &[
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
        ],
    );
    let plan = planner.plan(&diagnosis);
    assert_eq!(plan.cause, StagnationCause::ScopeTooLarge);
    assert_eq!(plan.strategy, RecoveryStrategy::ReduceScope);
}

#[test]
fn recovery_plan_for_external_dependency_broken() {
    let planner = RecoveryPlanner::new();
    let err = "connection refused";
    let diagnosis = DiagnosisEngine::new(0.3).diagnose(
        &(1..=5).map(make_flat_metrics).collect::<Vec<_>>(),
        &[
            vec![make_gate("test", false, err)],
            vec![make_gate("test", false, err)],
            vec![make_gate("test", false, err)],
        ],
        &[
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
        ],
    );
    let plan = planner.plan(&diagnosis);
    assert_eq!(plan.cause, StagnationCause::ExternalDependencyBroken);
    assert_eq!(plan.strategy, RecoveryStrategy::EscalateToHuman);
    assert_eq!(plan.risk_level, RiskLevel::High);
}

#[test]
fn recovery_plan_for_circular_fix() {
    let planner = RecoveryPlanner::new();
    let diagnosis = DiagnosisEngine::new(0.3).diagnose(
        &(1..=5).map(make_flat_metrics).collect::<Vec<_>>(),
        &[
            vec![make_gate("test", true, "")],
            vec![make_gate("test", true, "")],
            vec![make_gate("test", true, "")],
        ],
        &[
            vec!["src/lib.rs".to_string()],
            vec![],
            vec!["src/lib.rs".to_string()],
        ],
    );
    let plan = planner.plan(&diagnosis);
    assert_eq!(plan.cause, StagnationCause::CircularFix);
    assert_eq!(plan.strategy, RecoveryStrategy::RefactorApproach);
}

#[test]
fn recovery_plan_for_inefficient_exploration() {
    let planner = RecoveryPlanner::new();
    let mut history: Vec<_> = (1..=5).map(make_flat_metrics).collect();
    for m in &mut history {
        m.tokens_spent = 50000;
        m.proof_score = 0.3;
    }
    let diagnosis = DiagnosisEngine::new(0.3).diagnose(
        &history,
        &[
            vec![make_gate("test", true, "")],
            vec![make_gate("test", true, "")],
            vec![make_gate("test", true, "")],
        ],
        &[
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
        ],
    );
    let plan = planner.plan(&diagnosis);
    assert_eq!(plan.cause, StagnationCause::InefficientExploration);
    assert_eq!(plan.strategy, RecoveryStrategy::ReduceScope);
}

#[test]
fn recovery_plan_for_unknown() {
    let planner = RecoveryPlanner::new();
    // Create history that doesn't trigger any heuristic strongly.
    let history = vec![
        IterationMetrics {
            iteration: 1,
            proof_score: 0.5,
            commit_velocity: 2,
            gate_pass_rate: 0.5,
            coverage_delta: Some(0.5),
            tokens_spent: 100,
            files_touched: 3,
            timestamp: Utc::now(),
        },
        IterationMetrics {
            iteration: 2,
            proof_score: 0.51,
            commit_velocity: 2,
            gate_pass_rate: 0.51,
            coverage_delta: Some(0.5),
            tokens_spent: 100,
            files_touched: 3,
            timestamp: Utc::now(),
        },
    ];
    let diagnosis = DiagnosisEngine::new(0.3).diagnose(
        &history,
        &[
            vec![make_gate("test", true, "")],
            vec![make_gate("test", true, "")],
        ],
        &[
            vec!["src/lib.rs".to_string()],
            vec!["src/lib.rs".to_string()],
        ],
    );
    let plan = planner.plan(&diagnosis);
    assert_eq!(plan.cause, StagnationCause::Unknown);
    assert_eq!(plan.strategy, RecoveryStrategy::ReduceScope);
}

#[tokio::test]
async fn checkpoint_save_and_load_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let checkpoints_dir = tmp.path().join("checkpoints");

    let proof = make_proof(0.5);
    let budget = crate::runtime::goal::budget::GoalBudgetReport {
        version: 1,
        goal_id: "test-goal".to_string(),
        generated_at: Utc::now(),
        budget_time: None,
        total_budget_secs: None,
        budget_tokens: None,
        used_tokens: 1000,
        remaining_budget_tokens: None,
        budget_usd: None,
        estimated_cost_usd: 0.0,
        remaining_budget_usd: None,
        latest: None,
        checkpoints: Vec::new(),
        spent_usd: 0.0,
        spent_tokens: 0,
        spent_seconds: 0,
    };
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "test-goal".to_string(),
        generated_at: Utc::now(),
        tasks: vec![GoalTask {
            id: "task-1".to_string(),
            title: "Task 1".to_string(),
            description: "Description".to_string(),
            status: GoalTaskStatus::Pending,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: Vec::new(),
            read_set: Vec::new(),
            write_set: Vec::new(),
            risk: "low".to_string(),
            acceptance: vec!["ok".to_string()],
        }],
    };

    let checkpoint = RecoveryCheckpoint::from_state(
        1,
        "test-goal".to_string(),
        "abc123".to_string(),
        proof.clone(),
        &task_graph,
        budget.clone(),
    )
    .expect("create checkpoint");

    checkpoint
        .save(&checkpoints_dir)
        .await
        .expect("save checkpoint");

    let loaded = RecoveryCheckpoint::load(&checkpoints_dir, 1)
        .await
        .expect("load checkpoint");

    assert_eq!(loaded.checkpoint_id, 1);
    assert_eq!(loaded.goal_id, "test-goal");
    assert_eq!(loaded.git_commit, "abc123");
    assert_eq!(loaded.proof_snapshot.goal_id, proof.goal_id);

    let loaded_graph = loaded.task_graph().expect("deserialize task graph");
    assert_eq!(loaded_graph.tasks.len(), 1);
    assert_eq!(loaded_graph.tasks[0].id, "task-1");

    let ids = list_checkpoints(&checkpoints_dir)
        .await
        .expect("list checkpoints");
    assert_eq!(ids, vec![1]);
}

#[tokio::test]
async fn collector_save_and_load_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("history.jsonl");

    let mut collector = StagnationCollector::new(5);
    for i in 1..=3 {
        collector.record(make_flat_metrics(i)).unwrap();
    }

    collector.save(&path).await.expect("save history");
    let loaded = StagnationCollector::load(&path)
        .await
        .expect("load history");

    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].iteration, 1);
    assert_eq!(loaded[2].iteration, 3);
}

#[test]
fn collector_eviction_at_capacity() {
    let mut collector = StagnationCollector::new(3);
    collector.record(make_flat_metrics(1)).unwrap();
    collector.record(make_flat_metrics(2)).unwrap();
    collector.record(make_flat_metrics(3)).unwrap();
    collector.record(make_flat_metrics(4)).unwrap();

    let history = collector.history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].iteration, 2);
    assert_eq!(history[2].iteration, 4);
}

#[test]
fn collector_build_metrics_with_previous() {
    let collector = StagnationCollector::default();
    let proof = make_proof(0.5);
    let prev_proof = make_proof(0.5);
    let budget = make_budget(2000);
    let prev_budget = make_budget(1000);
    let gates = vec![make_gate("test", true, "")];
    let changed = vec!["src/lib.rs".to_string()];

    let metrics = collector
        .build_metrics(
            2,
            &proof,
            &budget,
            &gates,
            &changed,
            Some(&prev_proof),
            Some(&prev_budget),
        )
        .expect("build metrics");

    assert_eq!(metrics.iteration, 2);
    assert_eq!(metrics.tokens_spent, 1000);
}

#[test]
fn detector_empty_history() {
    let detector = StagnationDetector::default();
    let report = detector.detect(&[], GoalStatus::Running, GoalPhase::Execution);
    assert!(
        report.is_none(),
        "empty history should not trigger detection"
    );
}

#[test]
fn detector_ready_status() {
    let detector = StagnationDetector::default();
    let history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    let report = detector.detect(&history, GoalStatus::Ready, GoalPhase::Execution);
    assert!(
        report.is_none(),
        "ready status should not trigger detection"
    );
}

#[test]
fn levenshtein_identical_strings() {
    use crate::runtime::goal::stagnation::diagnosis::normalized_levenshtein;
    assert_eq!(normalized_levenshtein("abc", "abc"), 1.0);
}

#[test]
fn levenshtein_empty_string() {
    use crate::runtime::goal::stagnation::diagnosis::normalized_levenshtein;
    assert_eq!(normalized_levenshtein("", "abc"), 0.0);
    assert_eq!(normalized_levenshtein("abc", ""), 0.0);
}

#[test]
fn levenshtein_completely_different() {
    use crate::runtime::goal::stagnation::diagnosis::normalized_levenshtein;
    let sim = normalized_levenshtein("abc", "xyz");
    assert!((0.0..0.5).contains(&sim));
}

#[test]
fn collector_validates_metric_bounds() {
    let mut collector = StagnationCollector::default();
    let mut metrics = make_flat_metrics(1);
    metrics.proof_score = 1.5;
    let result = collector.record(metrics);
    assert!(result.is_err());
}

#[test]
fn collector_builds_metrics() {
    let collector = StagnationCollector::default();
    let proof = make_proof(0.5);
    let budget = make_budget(1000);
    let gates = vec![make_gate("test", true, "")];
    let changed = vec!["src/lib.rs".to_string()];

    let metrics = collector
        .build_metrics(1, &proof, &budget, &gates, &changed, None, None)
        .expect("build metrics");

    assert_eq!(metrics.iteration, 1);
    assert_eq!(metrics.proof_score, 1.0);
    assert_eq!(metrics.gate_pass_rate, 1.0);
    assert_eq!(metrics.tokens_spent, 1000);
    assert_eq!(metrics.files_touched, 1);
}

#[test]
fn detector_respects_blocked_on_human() {
    let detector = StagnationDetector::default();
    let history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    let report = detector.detect(&history, GoalStatus::BlockedOnHuman, GoalPhase::Execution);
    assert!(
        report.is_none(),
        "blocked on human should not flag stagnation"
    );
}

#[test]
fn detector_respects_planning_phase() {
    let detector = StagnationDetector::default();
    let history: Vec<_> = (1..=8).map(make_flat_metrics).collect();
    let report = detector.detect(&history, GoalStatus::Running, GoalPhase::Planning);
    assert!(
        report.is_none(),
        "planning phase should not flag stagnation"
    );
}
