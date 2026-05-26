use serde::{Deserialize, Serialize};

use crate::runtime::config::StagnationThresholdsConfig;
use crate::runtime::goal::state::{GoalPhase, GoalStatus};

use super::collector::IterationMetrics;

/// Thresholds for determining whether individual metrics are stagnant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StagnationThresholds {
    pub proof_score_epsilon: f64,
    pub commit_velocity_min: u32,
    pub gate_pass_rate_epsilon: f64,
    pub coverage_epsilon: f64,
    pub token_efficiency_max: f64,
    pub file_churn_max: u32,
}

impl Default for StagnationThresholds {
    fn default() -> Self {
        Self {
            proof_score_epsilon: 0.01,
            commit_velocity_min: 1,
            gate_pass_rate_epsilon: 0.05,
            coverage_epsilon: 0.01,
            token_efficiency_max: 1000.0,
            file_churn_max: 10,
        }
    }
}

impl From<StagnationThresholdsConfig> for StagnationThresholds {
    fn from(config: StagnationThresholdsConfig) -> Self {
        Self {
            proof_score_epsilon: config.proof_score_epsilon,
            commit_velocity_min: config.commit_velocity_min,
            gate_pass_rate_epsilon: config.gate_pass_rate_epsilon,
            coverage_epsilon: config.coverage_epsilon,
            token_efficiency_max: config.token_efficiency_max,
            file_churn_max: config.file_churn_max,
        }
    }
}

/// Report produced when stagnation is detected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StagnationReport {
    pub stagnant_metrics: Vec<String>,
    pub window_size: usize,
    pub analyzed_iterations: Vec<u32>,
}

/// Detects stagnation by analyzing a sliding window of iteration metrics.
#[derive(Debug, Clone)]
pub struct StagnationDetector {
    pub window_size: usize,
    pub min_stagnant_metrics: usize,
    pub thresholds: StagnationThresholds,
    pub warmup_iterations: usize,
}

impl Default for StagnationDetector {
    fn default() -> Self {
        Self {
            window_size: 5,
            min_stagnant_metrics: 3,
            thresholds: StagnationThresholds::default(),
            warmup_iterations: 3,
        }
    }
}

impl StagnationDetector {
    pub fn new(
        window_size: usize,
        min_stagnant_metrics: usize,
        thresholds: StagnationThresholds,
        warmup_iterations: usize,
    ) -> Self {
        Self {
            window_size,
            min_stagnant_metrics,
            thresholds,
            warmup_iterations,
        }
    }

    /// Analyze the metric history and return a stagnation report if detected.
    pub fn detect(
        &self,
        history: &[IterationMetrics],
        status: GoalStatus,
        phase: GoalPhase,
    ) -> Option<StagnationReport> {
        // Edge cases: do not flag in terminal or waiting states.
        if status == GoalStatus::Ready {
            return None;
        }
        if status == GoalStatus::BlockedOnHuman {
            return None;
        }
        if phase == GoalPhase::Planning {
            return None;
        }
        if history.len() < self.warmup_iterations + self.window_size {
            return None;
        }

        let window = &history[history.len().saturating_sub(self.window_size)..];
        if window.is_empty() {
            return None;
        }

        // If proof is complete, not stagnant.
        if let Some(last) = window.last() {
            if (1.0 - last.proof_score) <= self.thresholds.proof_score_epsilon {
                return None;
            }
        }

        let mut stagnant = Vec::new();

        if self.is_proof_score_stagnant(window) {
            stagnant.push("proof_score".to_string());
        }
        if self.is_commit_velocity_stagnant(window) {
            stagnant.push("commit_velocity".to_string());
        }
        if self.is_gate_pass_rate_stagnant(window) {
            stagnant.push("gate_pass_rate".to_string());
        }
        if self.is_coverage_delta_stagnant(window) {
            stagnant.push("coverage_delta".to_string());
        }
        if self.is_token_efficiency_stagnant(window) {
            stagnant.push("token_efficiency".to_string());
        }
        if self.is_file_churn_stagnant(window) {
            stagnant.push("file_churn".to_string());
        }

        if stagnant.len() >= self.min_stagnant_metrics {
            let analyzed_iterations = window.iter().map(|m| m.iteration).collect();
            Some(StagnationReport {
                stagnant_metrics: stagnant,
                window_size: self.window_size,
                analyzed_iterations,
            })
        } else {
            None
        }
    }

    fn is_proof_score_stagnant(&self, window: &[IterationMetrics]) -> bool {
        let scores: Vec<f64> = window.iter().map(|m| m.proof_score).collect();
        if scores.len() < 2 {
            return false;
        }
        let min = scores
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        let max = scores
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        (max - min) < self.thresholds.proof_score_epsilon
    }

    fn is_commit_velocity_stagnant(&self, window: &[IterationMetrics]) -> bool {
        let sum: u32 = window.iter().map(|m| m.commit_velocity).sum();
        sum < self.thresholds.commit_velocity_min * window.len() as u32
    }

    fn is_gate_pass_rate_stagnant(&self, window: &[IterationMetrics]) -> bool {
        let rates: Vec<f64> = window.iter().map(|m| m.gate_pass_rate).collect();
        if rates.len() < 2 {
            return false;
        }
        let min = rates
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        let max = rates
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        (max - min) < self.thresholds.gate_pass_rate_epsilon
    }

    fn is_coverage_delta_stagnant(&self, window: &[IterationMetrics]) -> bool {
        let deltas: Vec<f64> = window.iter().filter_map(|m| m.coverage_delta).collect();
        if deltas.is_empty() {
            // No coverage data available — do not count as stagnant.
            return false;
        }
        let total_delta: f64 = deltas.iter().sum();
        total_delta.abs() < self.thresholds.coverage_epsilon
    }

    fn is_token_efficiency_stagnant(&self, window: &[IterationMetrics]) -> bool {
        let median = median_f64(
            &window
                .iter()
                .map(|m| {
                    let gain = (m.proof_score * 100.0).max(1.0);
                    m.tokens_spent as f64 / gain
                })
                .collect::<Vec<_>>(),
        );
        median > self.thresholds.token_efficiency_max
    }

    fn is_file_churn_stagnant(&self, window: &[IterationMetrics]) -> bool {
        let median = median_u32(&window.iter().map(|m| m.files_touched).collect::<Vec<_>>());
        median > self.thresholds.file_churn_max
    }
}

fn median_f64(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn median_u32(values: &[u32]) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}
