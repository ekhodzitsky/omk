use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::runtime::goal::review::pass::ReviewPass;
use crate::runtime::goal::review::slice::{
    SliceReviewArtifact, SliceReviewContext, SliceReviewOutcome,
};

/// Performance review pass — detects benchmark regressions by comparing
/// current criterion estimates against a recorded baseline.
///
/// Defaults:
/// - `worktree_path`: `std::env::current_dir()` or `"."`
/// - `regression_threshold_pct`: 5.0  (mean estimate increase > 5% =
///   regression finding)
/// - `tracked_benches`: empty → discover from `benches/*.rs` filenames
/// - `baseline_path`: `<worktree>/proof/baselines/perf.json`
pub struct PerformanceReviewPass {
    worktree_path: PathBuf,
    regression_threshold_pct: f64,
    tracked_benches: Vec<String>,
    baseline_path: Option<PathBuf>,
    /// Test injection: bypass file I/O and use these maps directly.
    injected_baseline: Option<BaselineMap>,
    injected_current: Option<BaselineMap>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BaselineMap {
    // bench_id → mean estimate in nanoseconds
    entries: BTreeMap<String, f64>,
}

#[allow(dead_code)]
impl PerformanceReviewPass {
    pub fn new() -> Self {
        let worktree_path = match std::env::current_dir() {
            Ok(p) => p,
            Err(_) => PathBuf::from("."),
        };
        Self {
            worktree_path,
            regression_threshold_pct: 5.0,
            tracked_benches: Vec::new(),
            baseline_path: None,
            injected_baseline: None,
            injected_current: None,
        }
    }

    pub fn with_worktree_path(mut self, p: impl AsRef<Path>) -> Self {
        self.worktree_path = p.as_ref().to_path_buf();
        self
    }

    pub fn with_regression_threshold_pct(mut self, pct: f64) -> Self {
        self.regression_threshold_pct = pct;
        self
    }

    pub fn with_tracked_benches(mut self, names: Vec<String>) -> Self {
        self.tracked_benches = names;
        self
    }

    pub fn with_baseline_path(mut self, p: impl AsRef<Path>) -> Self {
        self.baseline_path = Some(p.as_ref().to_path_buf());
        self
    }

    #[cfg(test)]
    pub(crate) fn with_injected_baseline(mut self, m: BaselineMap) -> Self {
        self.injected_baseline = Some(m);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_injected_current(mut self, m: BaselineMap) -> Self {
        self.injected_current = Some(m);
        self
    }

    fn resolve_baseline(&self) -> (BaselineMap, Option<String>) {
        if let Some(baseline) = &self.injected_baseline {
            return (baseline.clone(), None);
        }
        let path = match &self.baseline_path {
            Some(p) => p.clone(),
            None => self
                .worktree_path
                .join("proof")
                .join("baselines")
                .join("perf.json"),
        };
        match read_baseline_file(&path) {
            Ok(Some(b)) => (b, None),
            Ok(None) => (BaselineMap::default(), None),
            Err(e) => (BaselineMap::default(), Some(e)),
        }
    }

    fn resolve_current(&self) -> BaselineMap {
        if let Some(current) = &self.injected_current {
            return current.clone();
        }
        let benches = if self.tracked_benches.is_empty() {
            discover_benches(&self.worktree_path)
        } else {
            self.tracked_benches.clone()
        };
        let mut entries = BTreeMap::new();
        for bench_id in benches {
            if let Some(ns) = read_criterion_estimate(&self.worktree_path, &bench_id) {
                entries.insert(bench_id, ns);
            }
        }
        BaselineMap { entries }
    }
}

impl Default for PerformanceReviewPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewPass for PerformanceReviewPass {
    fn name(&self) -> &'static str {
        "performance"
    }

    fn run(&self, _ctx: &SliceReviewContext) -> SliceReviewOutcome {
        let (baseline, baseline_err) = self.resolve_baseline();
        let current = self.resolve_current();

        if baseline.entries.is_empty() && current.entries.is_empty() {
            let msg = if let Some(e) = baseline_err {
                format!("Performance review: no benchmark data ({e})")
            } else {
                "Performance review: no benchmark data".to_string()
            };
            return build_performance_outcome(true, msg.clone(), Some(msg), "low");
        }

        if baseline.entries.is_empty() {
            let mut msg = "Performance review: no baseline recorded; record one to enable regression detection".to_string();
            if let Some(e) = baseline_err {
                msg.push_str(&format!(" ({e})"));
            }
            return build_performance_outcome(true, msg.clone(), Some(msg), "low");
        }

        if current.entries.is_empty() {
            return build_performance_outcome(
                true,
                "Performance review: no current bench data; nothing to compare".to_string(),
                Some("Performance review: no current bench data; nothing to compare".to_string()),
                "low",
            );
        }

        let mut findings: Vec<String> = Vec::new();
        for (bench_id, baseline_ns) in &baseline.entries {
            if let Some(current_ns) = current.entries.get(bench_id) {
                if *baseline_ns > 0.0 {
                    let delta_pct = ((current_ns - baseline_ns) / baseline_ns) * 100.0;
                    // Strictly greater than threshold counts as regression.
                    if delta_pct > self.regression_threshold_pct {
                        findings.push(format!(
                            "regression in {}: {:.0} ns → {:.0} ns (+{:.1}%)",
                            bench_id, baseline_ns, current_ns, delta_pct
                        ));
                    }
                }
            }
        }

        let passed = findings.is_empty();
        let feedback = if passed {
            format!(
                "Performance review passed: {} benchmark(s) within {:.1}% threshold",
                baseline.entries.len().min(current.entries.len()),
                self.regression_threshold_pct
            )
        } else {
            format!("Performance review blocked: {}", findings.join("; "))
        };
        let severity = if passed { "low" } else { "medium" };

        build_performance_outcome(
            passed,
            feedback.clone(),
            if passed { None } else { Some(feedback) },
            severity,
        )
    }
}

fn build_performance_outcome(
    passed: bool,
    artifact_feedback: String,
    outcome_feedback: Option<String>,
    severity: &str,
) -> SliceReviewOutcome {
    SliceReviewOutcome {
        passed,
        review_path: None,
        security_review_path: None,
        feedback: outcome_feedback,
        artifacts: vec![SliceReviewArtifact {
            kind: "performance".to_string(),
            passed,
            feedback: artifact_feedback,
            severity: severity.to_string(),
        }],
        slop_findings: Vec::new(),
    }
}

fn discover_benches(worktree: &Path) -> Vec<String> {
    let benches_dir = worktree.join("benches");
    let mut names = Vec::new();
    if let Ok(dir_entries) = std::fs::read_dir(&benches_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().to_string());
                }
            }
        }
    }
    names.sort();
    names
}

fn read_baseline_file(path: &Path) -> Result<Option<BaselineMap>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read baseline: {e}"))?;
    let entries: BTreeMap<String, f64> =
        serde_json::from_str(&content).map_err(|e| format!("malformed baseline: {e}"))?;
    Ok(Some(BaselineMap { entries }))
}

fn read_criterion_estimate(worktree: &Path, bench_id: &str) -> Option<f64> {
    let top = worktree
        .join("target")
        .join("criterion")
        .join(bench_id)
        .join("new")
        .join("estimates.json");
    if let Ok(content) = std::fs::read_to_string(&top) {
        if let Some(val) = parse_estimates_json(&content) {
            return Some(val);
        }
    }

    // Walk one level of subdirectories and sum (aggregate).
    let bench_dir = worktree.join("target").join("criterion").join(bench_id);
    let mut sum = 0.0;
    let mut found = false;
    if let Ok(dir_entries) = std::fs::read_dir(&bench_dir) {
        for entry in dir_entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                let estimates = sub.join("new").join("estimates.json");
                if let Ok(content) = std::fs::read_to_string(&estimates) {
                    if let Some(val) = parse_estimates_json(&content) {
                        sum += val;
                        found = true;
                    }
                }
            }
        }
    }

    if found {
        Some(sum)
    } else {
        None
    }
}

fn parse_estimates_json(content: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(content).ok()?;
    v.get("mean")?.get("point_estimate")?.as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bm(entries: &[(&str, f64)]) -> BaselineMap {
        BaselineMap {
            entries: entries.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn name_is_stable() {
        assert_eq!("performance", PerformanceReviewPass::new().name());
    }

    #[test]
    fn passes_when_baseline_empty() {
        let pass = PerformanceReviewPass::new()
            .with_injected_baseline(bm(&[]))
            .with_injected_current(bm(&[("bench_a", 1000.0)]));
        let outcome = pass.run(&SliceReviewContext);
        assert!(outcome.passed);
        assert!(outcome.feedback.unwrap().contains("no baseline"));
    }

    #[test]
    fn passes_when_current_empty() {
        let pass = PerformanceReviewPass::new()
            .with_injected_baseline(bm(&[("bench_a", 1000.0)]))
            .with_injected_current(bm(&[]));
        let outcome = pass.run(&SliceReviewContext);
        assert!(outcome.passed);
        assert!(outcome.feedback.unwrap().contains("no current bench data"));
    }

    #[test]
    fn passes_when_no_regression() {
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[("bench_a", 1000.0)]))
            .with_injected_current(bm(&[("bench_a", 1020.0)]));
        let outcome = pass.run(&SliceReviewContext);
        assert!(outcome.passed);
        assert!(outcome
            .artifacts
            .iter()
            .any(|a| a.kind == "performance" && a.passed));
    }

    #[test]
    fn fails_when_regression_exceeds_threshold() {
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[("bench_a", 1000.0)]))
            .with_injected_current(bm(&[("bench_a", 1100.0)]));
        let outcome = pass.run(&SliceReviewContext);
        assert!(!outcome.passed);
        let fb = outcome.feedback.unwrap();
        assert!(fb.contains("regression in bench_a"), "{fb}");
        assert!(fb.contains("+10.0%"), "{fb}");
    }

    #[test]
    fn handles_multiple_benches_some_regressed() {
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[
                ("bench_a", 1000.0),
                ("bench_b", 500.0),
                ("bench_c", 200.0),
            ]))
            .with_injected_current(bm(&[
                ("bench_a", 1010.0),
                ("bench_b", 600.0),
                ("bench_c", 220.0),
            ]));
        let outcome = pass.run(&SliceReviewContext);
        assert!(!outcome.passed);
        let fb = outcome.feedback.unwrap();
        assert!(fb.contains("regression in bench_b"), "{fb}");
        assert!(fb.contains("regression in bench_c"), "{fb}");
        assert!(!fb.contains("regression in bench_a"), "{fb}");
    }

    #[test]
    fn handles_missing_bench_in_current_gracefully() {
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[("bench_a", 1000.0), ("bench_b", 500.0)]))
            .with_injected_current(bm(&[("bench_a", 1010.0)]));
        assert!(pass.run(&SliceReviewContext).passed);
    }

    #[test]
    fn handles_missing_baseline_for_some_benches() {
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[("bench_a", 1000.0)]))
            .with_injected_current(bm(&[("bench_a", 1010.0), ("bench_b", 5000.0)]));
        assert!(pass.run(&SliceReviewContext).passed);
    }

    #[test]
    fn threshold_boundary_at_exactly_threshold() {
        // Policy: strictly greater than threshold counts as regression.
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[("bench_a", 1000.0)]))
            .with_injected_current(bm(&[("bench_a", 1050.0)]));
        assert!(
            pass.run(&SliceReviewContext).passed,
            "exactly at threshold should pass"
        );
    }

    #[test]
    fn negative_delta_is_improvement_not_regression() {
        let pass = PerformanceReviewPass::new()
            .with_regression_threshold_pct(5.0)
            .with_injected_baseline(bm(&[("bench_a", 1000.0)]))
            .with_injected_current(bm(&[("bench_a", 800.0)]));
        assert!(pass.run(&SliceReviewContext).passed);
    }

    #[test]
    fn discover_benches_reads_filenames() {
        let tmp = tempfile::tempdir().unwrap();
        let benches = tmp.path().join("benches");
        std::fs::create_dir_all(&benches).unwrap();
        std::fs::write(benches.join("foo.rs"), "").unwrap();
        std::fs::write(benches.join("bar.rs"), "").unwrap();
        assert_eq!(discover_benches(tmp.path()), vec!["bar", "foo"]);
    }

    #[test]
    fn real_estimates_json_parsing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp
            .path()
            .join("target")
            .join("criterion")
            .join("test_bench")
            .join("new");
        std::fs::create_dir_all(&dir).unwrap();
        let json = r#"{"mean":{"point_estimate":12345.0,"confidence_interval":{"lower_bound":12000.0,"upper_bound":12600.0}}}"#;
        std::fs::write(dir.join("estimates.json"), json).unwrap();

        let pass = PerformanceReviewPass::new()
            .with_worktree_path(tmp.path())
            .with_tracked_benches(vec!["test_bench".to_string()]);
        let outcome = pass.run(&SliceReviewContext);
        assert!(outcome.passed);
        assert!(outcome.feedback.unwrap().contains("no baseline"));
    }
}
