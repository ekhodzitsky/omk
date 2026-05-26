use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::runtime::gates::GateResult;

use super::collector::IterationMetrics;

/// Identified root cause of stagnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StagnationCause {
    TestFlakiness,
    ScopeTooLarge,
    ExternalDependencyBroken,
    CircularFix,
    ReviewRejectionLoop,
    InefficientExploration,
    Unknown,
}

impl std::fmt::Display for StagnationCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            StagnationCause::TestFlakiness => "test flakiness",
            StagnationCause::ScopeTooLarge => "scope too large",
            StagnationCause::ExternalDependencyBroken => "external dependency broken",
            StagnationCause::CircularFix => "circular fix",
            StagnationCause::ReviewRejectionLoop => "review rejection loop",
            StagnationCause::InefficientExploration => "inefficient exploration",
            StagnationCause::Unknown => "unknown",
        };
        write!(f, "{name}")
    }
}

/// Diagnosis report with confidence and evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagnosisReport {
    pub cause: StagnationCause,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub affected_gates: Vec<String>,
    pub affected_files: Vec<String>,
}

/// Engine that diagnoses the root cause of stagnation.
#[derive(Debug, Clone, Default)]
pub struct DiagnosisEngine {
    min_confidence_threshold: f64,
}

impl DiagnosisEngine {
    pub fn new(min_confidence_threshold: f64) -> Self {
        Self {
            min_confidence_threshold,
        }
    }

    /// Run all heuristics and return the best diagnosis.
    pub fn diagnose(
        &self,
        history: &[IterationMetrics],
        gates_history: &[Vec<GateResult>],
        changed_files_history: &[Vec<String>],
    ) -> DiagnosisReport {
        // Validate parallel slice invariants.
        let expected_len = history.len();
        if gates_history.len() < expected_len || changed_files_history.len() < expected_len {
            tracing::warn!(
                history_len = expected_len,
                gates_history_len = gates_history.len(),
                changed_files_history_len = changed_files_history.len(),
                "diagnosis input slices have mismatched lengths"
            );
        }
        let window = history;
        #[allow(clippy::type_complexity)]
        let mut candidates: Vec<(
            StagnationCause,
            f64,
            Vec<String>,
            Vec<String>,
            Vec<String>,
        )> = Vec::new();

        if let Some((conf, evidence, affected_gates)) = detect_test_flakiness(window, gates_history)
        {
            candidates.push((
                StagnationCause::TestFlakiness,
                conf,
                evidence,
                affected_gates,
                Vec::new(),
            ));
        }
        if let Some((conf, evidence, affected_files)) = detect_scope_too_large(window) {
            candidates.push((
                StagnationCause::ScopeTooLarge,
                conf,
                evidence,
                Vec::new(),
                affected_files,
            ));
        }
        if let Some((conf, evidence, affected_gates)) =
            detect_external_dependency_broken(window, gates_history)
        {
            candidates.push((
                StagnationCause::ExternalDependencyBroken,
                conf,
                evidence,
                affected_gates,
                Vec::new(),
            ));
        }
        if let Some((conf, evidence, affected_files)) =
            detect_circular_fix(window, changed_files_history)
        {
            candidates.push((
                StagnationCause::CircularFix,
                conf,
                evidence,
                Vec::new(),
                affected_files,
            ));
        }
        if let Some((conf, evidence, _)) = detect_review_rejection_loop(window) {
            candidates.push((
                StagnationCause::ReviewRejectionLoop,
                conf,
                evidence,
                Vec::new(),
                Vec::new(),
            ));
        }
        if let Some((conf, evidence, _)) = detect_inefficient_exploration(window) {
            candidates.push((
                StagnationCause::InefficientExploration,
                conf,
                evidence,
                Vec::new(),
                Vec::new(),
            ));
        }

        // Select highest confidence cause.
        let best = candidates
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((cause, confidence, evidence, affected_gates, affected_files)) = best {
            if confidence >= self.min_confidence_threshold {
                return DiagnosisReport {
                    cause,
                    confidence,
                    evidence,
                    affected_gates,
                    affected_files,
                };
            }
        }

        // Fallback: Unknown with safe default evidence.
        let mut evidence = Vec::new();
        evidence.push("no clear stagnation pattern matched the heuristic thresholds".to_string());
        if let Some(last) = window.last() {
            evidence.push(format!(
                "last iteration {}: proof_score={}, gate_pass_rate={}, files_touched={}",
                last.iteration, last.proof_score, last.gate_pass_rate, last.files_touched
            ));
        }

        DiagnosisReport {
            cause: StagnationCause::Unknown,
            confidence: 0.0,
            evidence,
            affected_gates: Vec::new(),
            affected_files: Vec::new(),
        }
    }
}

/// Detect alternating pass/fail for the same gate across iterations.
fn detect_test_flakiness(
    window: &[IterationMetrics],
    gates_history: &[Vec<GateResult>],
) -> Option<(f64, Vec<String>, Vec<String>)> {
    if gates_history.len() < 2 {
        return None;
    }
    let relevant = &gates_history[gates_history.len().saturating_sub(window.len())..];
    if relevant.len() < 2 {
        return None;
    }

    let mut flip_counts: HashMap<String, u32> = HashMap::new();
    let mut total_runs: HashMap<String, u32> = HashMap::new();

    for window in relevant.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        for gate in curr {
            let prev_state = prev.iter().find(|g| g.name == gate.name).map(|g| g.passed);
            if let Some(prev_passed) = prev_state {
                *total_runs.entry(gate.name.clone()).or_insert(0) += 1;
                if prev_passed != gate.passed {
                    *flip_counts.entry(gate.name.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut best_gate = None;
    let mut best_confidence = 0.0;
    let mut evidence = Vec::new();

    for (gate_name, flips) in &flip_counts {
        let total = total_runs.get(gate_name).copied().unwrap_or(1).max(1);
        let confidence = (*flips as f64) / (total as f64);
        if confidence > best_confidence {
            best_confidence = confidence;
            best_gate = Some(gate_name.clone());
        }
    }

    if best_confidence >= 0.5 {
        if let Some(ref gate_name) = best_gate {
            evidence.push(format!(
                "gate '{}' alternates pass/fail across iterations (flips={}/{})",
                gate_name,
                flip_counts.get(gate_name).copied().unwrap_or(0),
                total_runs.get(gate_name).copied().unwrap_or(1)
            ));
        }
        Some((best_confidence, evidence, best_gate.into_iter().collect()))
    } else {
        None
    }
}

/// Detect high file churn combined with low and flat proof score.
fn detect_scope_too_large(window: &[IterationMetrics]) -> Option<(f64, Vec<String>, Vec<String>)> {
    if window.len() < 2 {
        return None;
    }
    let median_files = median_u32(&window.iter().map(|m| m.files_touched).collect::<Vec<_>>());
    let proof_flat = window.last().map(|m| m.proof_score).unwrap_or(1.0)
        - window.first().map(|m| m.proof_score).unwrap_or(0.0);
    let proof_low = window.last().map(|m| m.proof_score).unwrap_or(1.0) < 0.5;

    if median_files > 10 && proof_low && proof_flat.abs() < 0.01 {
        let confidence = (median_files as f64 / 20.0).min(1.0);
        let evidence = vec![format!(
            "high file churn (median={} files) with low flat proof_score ({:.2})",
            median_files,
            window.last().map(|m| m.proof_score).unwrap_or(0.0)
        )];
        Some((confidence, evidence, Vec::new()))
    } else {
        None
    }
}

/// Maximum characters to compare for stderr similarity.
const MAX_STDERR_SIMILARITY_LEN: usize = 500;

/// Detect a gate failing with identical stderr across ≥3 consecutive iterations.
fn detect_external_dependency_broken(
    window: &[IterationMetrics],
    gates_history: &[Vec<GateResult>],
) -> Option<(f64, Vec<String>, Vec<String>)> {
    let relevant = &gates_history[gates_history.len().saturating_sub(window.len())..];
    if relevant.len() < 3 {
        return None;
    }

    // Group failing gates by name to avoid O(n²) over all gates.
    let mut failures_by_gate: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for gate_results in relevant.iter().flatten() {
        if !gate_results.passed && !gate_results.stderr.is_empty() {
            let capped = truncate(&gate_results.stderr, MAX_STDERR_SIMILARITY_LEN);
            failures_by_gate
                .entry(gate_results.name.clone())
                .or_default()
                .push(capped);
        }
    }

    let mut best_gate = None;
    let mut best_streak = 0;
    let mut best_error = String::new();

    for (gate_name, stderrs) in &failures_by_gate {
        if stderrs.len() < 3 {
            continue;
        }
        let mut streak = 1;
        let mut max_streak = 1;
        let representative = stderrs[0].clone();
        for i in 1..stderrs.len() {
            let similarity = normalized_levenshtein(&stderrs[i - 1], &stderrs[i]);
            if similarity > 0.9 {
                streak += 1;
                max_streak = max_streak.max(streak);
            } else {
                streak = 1;
            }
        }
        if max_streak >= 3 && max_streak > best_streak {
            best_streak = max_streak;
            best_gate = Some(gate_name.clone());
            best_error = representative;
        }
    }

    if best_streak >= 3 {
        let confidence = (best_streak as f64 / 5.0).min(1.0);
        let evidence = vec![format!(
            "gate '{}' failed {} times with similar error: '{}'",
            best_gate.as_deref().unwrap_or("?"),
            best_streak,
            truncate(&best_error, 120)
        )];
        Some((confidence, evidence, best_gate.into_iter().collect()))
    } else {
        None
    }
}

/// Detect files appearing, disappearing, and reappearing across iterations.
fn detect_circular_fix(
    window: &[IterationMetrics],
    changed_files_history: &[Vec<String>],
) -> Option<(f64, Vec<String>, Vec<String>)> {
    let relevant =
        &changed_files_history[changed_files_history.len().saturating_sub(window.len())..];
    if relevant.len() < 3 {
        return None;
    }

    let mut cycles = 0;
    let mut cycled_files = Vec::new();

    for i in 0..relevant.len().saturating_sub(2) {
        let set_n: std::collections::HashSet<&str> =
            relevant[i].iter().map(|s| s.as_str()).collect();
        let set_n1: std::collections::HashSet<&str> =
            relevant[i + 1].iter().map(|s| s.as_str()).collect();
        let set_n2: std::collections::HashSet<&str> =
            relevant[i + 2].iter().map(|s| s.as_str()).collect();

        for file in &set_n {
            if !set_n1.contains(file) && set_n2.contains(file) {
                cycles += 1;
                cycled_files.push(file.to_string());
            }
        }
    }

    if cycles >= 1 {
        let confidence = (cycles as f64 / 3.0).min(1.0);
        let mut evidence = Vec::new();
        evidence.push(format!("detected {cycles} file modification cycle(s)"));
        for file in cycled_files.iter().take(3) {
            evidence.push(format!(
                "  - file '{file}' modified, reverted, then modified again"
            ));
        }
        Some((confidence, evidence, cycled_files))
    } else {
        None
    }
}

/// Placeholder for review rejection loop detection.
/// Requires review task metadata which is not directly available in the metrics window.
fn detect_review_rejection_loop(
    _window: &[IterationMetrics],
) -> Option<(f64, Vec<String>, Vec<String>)> {
    // Without review task status history, we cannot reliably detect this.
    // Return None so the diagnosis falls back to other heuristics or Unknown.
    None
}

/// Detect high token efficiency with flat proof score.
fn detect_inefficient_exploration(
    window: &[IterationMetrics],
) -> Option<(f64, Vec<String>, Vec<String>)> {
    if window.len() < 2 {
        return None;
    }
    let proof_flat = window.last().map(|m| m.proof_score).unwrap_or(1.0)
        - window.first().map(|m| m.proof_score).unwrap_or(0.0);
    let median_efficiency = median_f64(
        &window
            .iter()
            .map(|m| {
                let gain = (m.proof_score * 100.0).max(1.0);
                m.tokens_spent as f64 / gain
            })
            .collect::<Vec<_>>(),
    );

    if proof_flat.abs() < 0.01 && median_efficiency > 1000.0 {
        let confidence = ((median_efficiency / 1000.0) - 1.0).min(1.0);
        let evidence = vec![format!(
            "high token efficiency ({:.0} tokens per 1% progress) with flat proof_score",
            median_efficiency
        )];
        Some((confidence, evidence, Vec::new()))
    } else {
        None
    }
}

pub(crate) fn normalized_levenshtein(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let distance = levenshtein(a, b);
    let max_len = a.len().max(b.len()) as f64;
    1.0 - (distance as f64 / max_len)
}

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for (i, a_ch) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_ch) in b.chars().enumerate() {
            let cost = if a_ch == b_ch { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
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
