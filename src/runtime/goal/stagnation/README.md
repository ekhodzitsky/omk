# Stagnation Recovery Module

Adaptive stagnation detection and recovery for OMK goal execution.

## Purpose

Detects when a goal agent is stuck in a broken loop **before** budget exhaustion.
Analyzes iteration metrics over a sliding window, diagnoses root cause, and proposes
operator-approved recovery plans.

## Public API

### Core Types

- `StagnationCollector` — collects `IterationMetrics` from goal artifacts
- `StagnationDetector` — analyzes metric history for stagnation patterns
- `DiagnosisEngine` — runs heuristics to identify root cause
- `RecoveryPlanner` — generates `RecoveryPlan` from diagnosis
- `RecoveryCheckpoint` — snapshots goal state for rollback

### Entry Points

```rust
// Detection
let detector = StagnationDetector::default();
let report = detector.detect(&history, status, phase);

// Diagnosis
let engine = DiagnosisEngine::new(0.3);
let diagnosis = engine.diagnose(&history, &gates_history, &changed_files_history);

// Recovery planning
let planner = RecoveryPlanner::new();
let plan = planner.plan(&diagnosis);

// Checkpointing
let checkpoint = RecoveryCheckpoint::from_state(id, goal_id, git_commit, proof, task_graph, budget)?;
checkpoint.save(&checkpoints_dir).await?;
```

## Consumers

- `src/cli/goal/commands/stagnation.rs` — CLI surface (`diagnose`, `recover`, `rollback`)
- Future: `src/runtime/goal/lifecycle.rs` — automatic background detection after each iteration

## Invariants

1. `history.len() >= warmup_iterations + window_size` required for detection.
2. `coverage_delta == None` means "no data", not "stagnant".
3. Recovery plans are **proposed**, not auto-executed (bounded autonomy).
4. `max_recoveries_per_goal` is enforced in CLI; future lifecycle integration will enforce it too.

## Dependencies

- `crate::runtime::gates::GateResult`
- `crate::runtime::goal::budget::GoalBudgetCheckpoint`
- `crate::runtime::goal::proof::GoalProof`
- `crate::runtime::goal::task_graph::GoalTaskGraph`
- `crate::runtime::config::StagnationConfig`
