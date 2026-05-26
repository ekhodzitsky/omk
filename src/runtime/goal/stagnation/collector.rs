use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use thiserror::Error;

use crate::runtime::gates::GateResult;
use crate::runtime::goal::budget::GoalBudgetCheckpoint;
use crate::runtime::goal::proof::GoalProof;

/// Metrics recorded for a single goal execution iteration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IterationMetrics {
    pub iteration: u32,
    pub proof_score: f64,
    pub commit_velocity: u32,
    pub gate_pass_rate: f64,
    pub coverage_delta: Option<f64>,
    pub tokens_spent: u64,
    pub files_touched: u32,
    pub timestamp: DateTime<Utc>,
}

/// Error type for stagnation collector operations.
#[derive(Error, Debug)]
pub enum StagnationCollectorError {
    #[error("invalid metrics: {0}")]
    InvalidMetrics(String),
    #[error("io failed")]
    Io(#[source] std::io::Error),
    #[error("serialization failed")]
    Serialization(#[source] serde_json::Error),
}

/// Collects and stores iteration metrics for stagnation analysis.
#[derive(Debug, Clone)]
pub struct StagnationCollector {
    history: VecDeque<IterationMetrics>,
    max_history: usize,
}

impl Default for StagnationCollector {
    fn default() -> Self {
        Self::new(10)
    }
}

impl StagnationCollector {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Record metrics from a completed iteration.
    pub fn record(&mut self, metrics: IterationMetrics) -> Result<(), StagnationCollectorError> {
        if metrics.proof_score < 0.0 || metrics.proof_score > 1.0 {
            return Err(StagnationCollectorError::InvalidMetrics(
                "proof_score must be in [0.0, 1.0]".to_string(),
            ));
        }
        if metrics.gate_pass_rate < 0.0 || metrics.gate_pass_rate > 1.0 {
            return Err(StagnationCollectorError::InvalidMetrics(
                "gate_pass_rate must be in [0.0, 1.0]".to_string(),
            ));
        }
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(metrics);
        Ok(())
    }

    /// Return the full history as a cloned vector.
    pub fn history(&self) -> Vec<IterationMetrics> {
        self.history.iter().cloned().collect()
    }

    /// Clear all recorded history.
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Build iteration metrics from available goal artifacts.
    ///
    /// `previous_proof` and `previous_budget` are from the prior iteration (if any)
    /// to compute deltas. `current_iteration` is 1-indexed.
    #[allow(clippy::too_many_arguments)]
    pub fn build_metrics(
        &self,
        current_iteration: u32,
        proof: &GoalProof,
        budget: &GoalBudgetCheckpoint,
        gates: &[GateResult],
        changed_files: &[String],
        previous_proof: Option<&GoalProof>,
        previous_budget: Option<&GoalBudgetCheckpoint>,
    ) -> Result<IterationMetrics, StagnationCollectorError> {
        let proof_score = compute_proof_score(proof);
        let commit_velocity = compute_commit_velocity(proof, previous_proof);
        let gate_pass_rate = compute_gate_pass_rate(gates);
        let coverage_delta = None; // TODO: integrate with CI-065 Coverage Radar
        let tokens_spent = compute_tokens_spent(budget, previous_budget);
        let files_touched = changed_files.len() as u32;

        Ok(IterationMetrics {
            iteration: current_iteration,
            proof_score,
            commit_velocity,
            gate_pass_rate,
            coverage_delta,
            tokens_spent,
            files_touched,
            timestamp: Utc::now(),
        })
    }

    /// Persist history to a JSONL file.
    pub async fn save(&self, path: &Path) -> Result<(), StagnationCollectorError> {
        let mut lines = Vec::new();
        for metric in &self.history {
            let json =
                serde_json::to_string(metric).map_err(StagnationCollectorError::Serialization)?;
            lines.push(json);
        }
        let content = lines.join("\n");
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(StagnationCollectorError::Io)?;
        }
        tokio::fs::write(path, content)
            .await
            .map_err(StagnationCollectorError::Io)?;
        Ok(())
    }

    /// Load history from a JSONL file.
    pub async fn load(path: &Path) -> Result<Vec<IterationMetrics>, StagnationCollectorError> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(StagnationCollectorError::Io)?;
        let mut metrics = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let m: IterationMetrics =
                serde_json::from_str(line).map_err(StagnationCollectorError::Serialization)?;
            metrics.push(m);
        }
        Ok(metrics)
    }
}

fn compute_proof_score(proof: &GoalProof) -> f64 {
    if proof.gates.is_empty() {
        return 0.0;
    }
    let passed = proof.gates.iter().filter(|g| g.passed).count() as f64;
    let total = proof.gates.len() as f64;
    passed / total
}

fn compute_commit_velocity(proof: &GoalProof, previous: Option<&GoalProof>) -> u32 {
    match previous {
        Some(prev) => {
            let prev_commits: std::collections::HashSet<&str> =
                prev.commits.iter().map(|s| s.as_str()).collect();
            proof
                .commits
                .iter()
                .filter(|c| !prev_commits.contains(c.as_str()))
                .count() as u32
        }
        None => proof.commits.len() as u32,
    }
}

fn compute_gate_pass_rate(gates: &[GateResult]) -> f64 {
    if gates.is_empty() {
        return 0.0;
    }
    let passed = gates.iter().filter(|g| g.passed).count() as f64;
    let total = gates.len() as f64;
    passed / total
}

fn compute_tokens_spent(
    budget: &GoalBudgetCheckpoint,
    previous: Option<&GoalBudgetCheckpoint>,
) -> u64 {
    match previous {
        Some(prev) => budget.used_tokens.saturating_sub(prev.used_tokens),
        None => budget.used_tokens,
    }
}
