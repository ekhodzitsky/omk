use anyhow::{Context, Result};
use std::path::Path;

use crate::runtime::config::StagnationConfig;
use crate::runtime::goal::stagnation::checkpoint::{list_checkpoints, RecoveryCheckpoint};
use crate::runtime::goal::stagnation::collector::{IterationMetrics, StagnationCollector};
use crate::runtime::goal::stagnation::detector::{StagnationDetector, StagnationThresholds};
use crate::runtime::goal::stagnation::diagnosis::DiagnosisEngine;
use crate::runtime::goal::stagnation::recovery::RecoveryPlanner;
use crate::runtime::goal::{GoalBudgetCheckpoint, GoalProof, GoalStateStore, GoalTaskGraph};

const STAGNATION_HISTORY_FILE: &str = "stagnation_history.jsonl";

fn detector_from_config(config: &StagnationConfig) -> StagnationDetector {
    StagnationDetector::new(
        config.window_size,
        config.min_stagnant_metrics,
        StagnationThresholds::from(config.thresholds.clone()),
        config.warmup_iterations,
    )
}

async fn load_history(state_dir: &Path) -> Vec<IterationMetrics> {
    let path = state_dir.join(STAGNATION_HISTORY_FILE);
    StagnationCollector::load(&path).await.unwrap_or_default()
}

pub(crate) async fn cmd_diagnose(goal_id: &str) -> Result<()> {
    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let proof = GoalProof::load(&state.state_dir).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;

    let mut history = load_history(&state.state_dir).await;
    let history_loaded = !history.is_empty();

    // If no persisted history, build a single-iteration snapshot from current state.
    if history.is_empty() {
        let budget_report = crate::runtime::goal::goal_budget(goal_id).await?;
        let budget = GoalBudgetCheckpoint {
            version: 1,
            goal_id: state.goal_id.clone(),
            label: "diagnose".to_string(),
            status: state.status,
            phase: state.phase,
            recorded_at: chrono::Utc::now(),
            budget_time: state.budget_time.clone(),
            total_budget_secs: None,
            elapsed_since_created_secs: 0,
            remaining_budget_secs: None,
            budget_tokens: state.budget_tokens,
            used_tokens: budget_report.used_tokens,
            remaining_budget_tokens: None,
            budget_usd: state.budget_usd,
            estimated_cost_usd: budget_report.estimated_cost_usd,
            remaining_budget_usd: None,
        };

        let collector = StagnationCollector::default();
        let metrics = collector
            .build_metrics(
                1,
                &proof,
                &budget,
                &proof.gates,
                &proof.changed_files,
                None,
                None,
            )
            .context("failed to build stagnation metrics")?;
        history.push(metrics);
    }

    let detector = detector_from_config(&config.stagnation);
    let diagnosis_engine = DiagnosisEngine::new(0.3);
    let planner = RecoveryPlanner::new();

    println!("Stagnation analysis for goal {}", state.goal_id);
    println!();
    println!("History entries: {}", history.len());
    if let Some(last) = history.last() {
        println!("Last iteration metrics:");
        println!("  proof_score:       {:.2}", last.proof_score);
        println!("  gate_pass_rate:    {:.2}", last.gate_pass_rate);
        println!("  commit_velocity:   {}", last.commit_velocity);
        println!("  files_touched:     {}", last.files_touched);
        println!("  tokens_spent:      {}", last.tokens_spent);
    }
    println!();

    if let Some(report) = detector.detect(&history, state.status, state.phase) {
        tracing::info!(
            goal_id = %state.goal_id,
            stagnant_metrics = ?report.stagnant_metrics,
            "stagnation detected"
        );
        println!("[STAGNATION DETECTED]");
        for metric in &report.stagnant_metrics {
            println!("  - {metric} is stagnant");
        }

        // Diagnosis heuristics that require temporal gate/file data are skipped
        // when we only have a single-iteration snapshot or loaded history without
        // parallel gate/file artifacts.
        let (gates_history, changed_history): (Vec<Vec<_>>, Vec<Vec<_>>) = if history_loaded {
            tracing::info!(
                goal_id = %state.goal_id,
                "diagnosis running with metrics-only heuristics (no historical gate/file data)"
            );
            (Vec::new(), Vec::new())
        } else {
            (vec![proof.gates.clone()], vec![proof.changed_files.clone()])
        };

        let diagnosis = diagnosis_engine.diagnose(&history, &gates_history, &changed_history);
        println!();
        println!("[DIAGNOSIS] {}", diagnosis.cause);
        println!("  confidence: {:.2}", diagnosis.confidence);
        for evidence in &diagnosis.evidence {
            println!("  {evidence}");
        }

        let plan = planner.plan(&diagnosis);
        println!();
        println!("[RECOVERY PLAN] {}", plan.strategy);
        println!("  description: {}", plan.description);
        println!("  risk: {:?}", plan.risk_level);
        if let Some(tokens) = plan.estimated_tokens {
            println!("  estimated tokens: {tokens}");
        }
        for task in &plan.suggested_tasks {
            println!("  task: {}", task.description);
        }
    } else {
        println!("No stagnation detected (insufficient history or metrics improving).");
        println!();
        println!(
            "Note: full stagnation analysis requires at least {} iterations.",
            detector.window_size + detector.warmup_iterations
        );
    }

    println!();
    println!(
        "Tasks: {}/{} done",
        task_graph
            .tasks
            .iter()
            .filter(|t| t.status == crate::runtime::goal::GoalTaskStatus::Done)
            .count(),
        task_graph.tasks.len()
    );
    Ok(())
}

pub(crate) async fn cmd_recover(goal_id: &str, approve: bool) -> Result<()> {
    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut proof = GoalProof::load(&state.state_dir).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;

    if state.recovery_attempts >= config.stagnation.max_recoveries_per_goal {
        anyhow::bail!(
            "goal '{}' has reached the maximum recovery attempts ({})",
            state.goal_id,
            config.stagnation.max_recoveries_per_goal
        );
    }

    let checkpoints_dir = state.state_dir.join("checkpoints");
    let existing = list_checkpoints(&checkpoints_dir)
        .await
        .context("failed to list checkpoints")?;
    let next_id = existing.last().copied().unwrap_or(0) + 1;

    let budget = crate::runtime::goal::goal_budget(goal_id).await?;
    let checkpoint = RecoveryCheckpoint::from_state(
        next_id,
        state.goal_id.clone(),
        proof
            .commits
            .last()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        proof.clone(),
        &task_graph,
        budget,
    )
    .context("failed to create recovery checkpoint")?;

    checkpoint
        .save(&checkpoints_dir)
        .await
        .context("failed to save recovery checkpoint")?;

    tracing::info!(
        goal_id = %state.goal_id,
        checkpoint_id = checkpoint.checkpoint_id,
        "recovery checkpoint created"
    );

    println!(
        "Recovery checkpoint {} created for goal {}",
        checkpoint.checkpoint_id, state.goal_id
    );

    if approve {
        state.recovery_attempts += 1;
        proof.recovery_status = Some("recovery_in_progress".to_string());

        crate::runtime::goal::FileSystemGoalStateStore::new()
            .save(&state)
            .await
            .context("failed to update goal state")?;

        // Persist updated proof
        let proof_path = state.state_dir.join(crate::runtime::goal::GOAL_PROOF_FILE);
        let proof_json =
            serde_json::to_string_pretty(&proof).context("failed to serialize updated proof")?;
        tokio::fs::write(&proof_path, proof_json)
            .await
            .context("failed to write updated proof")?;

        tracing::info!(
            goal_id = %state.goal_id,
            recovery_attempts = state.recovery_attempts,
            "recovery approved"
        );

        println!(
            "Recovery approved. recovery_attempts={}",
            state.recovery_attempts
        );
        println!("Recovery tasks would be created here (integration pending).");
    } else {
        println!("Recovery proposed but not approved. Use --approve to execute.");
    }

    Ok(())
}

pub(crate) async fn cmd_rollback(goal_id: &str, checkpoint_id: u32) -> Result<()> {
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let checkpoints_dir = state.state_dir.join("checkpoints");
    let checkpoint = RecoveryCheckpoint::load(&checkpoints_dir, checkpoint_id)
        .await
        .with_context(|| format!("checkpoint {checkpoint_id} not found"))?;

    tracing::info!(
        goal_id = %state.goal_id,
        checkpoint_id,
        "rollback requested"
    );

    println!(
        "Rolling back goal {} to checkpoint {}",
        state.goal_id, checkpoint_id
    );
    println!("  git_commit: {}", checkpoint.git_commit);
    println!("  created_at: {}", checkpoint.created_at);
    println!();
    println!(
        "Proof snapshot status: {}",
        checkpoint.proof_snapshot.status
    );

    let task_graph = checkpoint
        .task_graph()
        .context("failed to restore task graph")?;
    println!("Task graph restored: {} tasks", task_graph.tasks.len());
    println!();
    println!("To complete rollback, run:");
    println!(
        "  git checkout {}  # if in a git repo",
        checkpoint.git_commit
    );
    println!("  # Then restore proof.json and task-graph.json from checkpoint");

    Ok(())
}
