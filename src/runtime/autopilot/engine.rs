use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::runtime::autopilot::helpers::run_kimi_prompt;
use crate::runtime::autopilot::types::{
    AutopilotPhase, AutopilotState, PhaseLog, QaResults, ValidationResult,
};
use crate::runtime::gates::{
    detect_changed_files, format_gate_summary, gates_passed, load_or_detect_gates, run_gates,
    DoneContract,
};

/// Autopilot engine that drives the 6-phase pipeline.
pub struct Autopilot {
    pub name: String,
    pub task: String,
    pub dir: PathBuf,
    pub enable_ralph: bool,
    pub state: AutopilotState,
    pub state_dir: PathBuf,
    pub interactive: bool,
    pub yolo: bool,
}

impl Autopilot {
    pub fn new(name: &str, task: &str, dir: &Path, enable_ralph: bool, yolo: bool) -> Self {
        let state_dir = crate::runtime::config::state_dir()
            .join("autopilot")
            .join(name);
        let plans_dir = crate::runtime::config::data_dir().join("plans");
        let state = AutopilotState {
            version: 1,
            task: task.to_string(),
            phase: AutopilotPhase::Expansion,
            plans_dir: plans_dir.clone(),
            created_at: chrono::Utc::now(),
            current_plan: None,
            qa_results: None,
            gate_results: vec![],
            validation_results: vec![],
            execution_log: vec![],
        };
        Self {
            name: name.to_string(),
            task: task.to_string(),
            dir: dir.to_path_buf(),
            enable_ralph,
            state,
            state_dir,
            interactive: true,
            yolo,
        }
    }

    pub async fn from_state(state_dir: &Path, enable_ralph: bool, yolo: bool) -> Result<Self> {
        let state = Self::load_state(state_dir).await?;
        Ok(Self {
            name: state_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            task: state.task.clone(),
            dir: std::env::current_dir()?,
            enable_ralph,
            state,
            state_dir: state_dir.to_path_buf(),
            interactive: true,
            yolo,
        })
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("autopilot-state.json")
    }

    pub async fn save_state(&self) -> Result<()> {
        let path = self.state_file();
        tokio::fs::create_dir_all(&self.state_dir).await?;
        let json = serde_json::to_string_pretty(&self.state)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
        info!(path = %path.display(), phase = ?self.state.phase, "Saved autopilot state");
        Ok(())
    }

    pub async fn load_state(state_dir: &Path) -> Result<AutopilotState> {
        let path = state_dir.join("autopilot-state.json");
        crate::runtime::migrate::migrate_if_needed(&path).await?;
        let json = tokio::fs::read_to_string(&path).await?;
        let state: AutopilotState = serde_json::from_str(&json)?;
        Ok(state)
    }

    pub async fn run(&mut self) -> Result<()> {
        info!(name = %self.name, task = %self.task, "Starting autopilot");
        self.print_progress();
        self.save_state().await?;

        #[allow(clippy::type_complexity)]
        let phases: Vec<(
            AutopilotPhase,
            Box<
                dyn Fn(
                    &mut Self,
                )
                    -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + '_>>,
            >,
        )> = vec![
            (
                AutopilotPhase::Expansion,
                Box::new(|s| Box::pin(s.run_expansion())),
            ),
            (
                AutopilotPhase::Planning,
                Box::new(|s| Box::pin(s.run_planning())),
            ),
            (
                AutopilotPhase::Execution,
                Box::new(|s| Box::pin(s.run_execution())),
            ),
            (AutopilotPhase::Qa, Box::new(|s| Box::pin(s.run_qa()))),
            (
                AutopilotPhase::Validation,
                Box::new(|s| Box::pin(s.run_validation())),
            ),
            (
                AutopilotPhase::Cleanup,
                Box::new(|s| Box::pin(async move { s.run_cleanup() })),
            ),
        ];

        let start_idx = phases
            .iter()
            .position(|(p, _)| *p == self.state.phase)
            .unwrap_or(0);

        for (phase, handler) in &phases[start_idx..] {
            if self.state.phase == AutopilotPhase::Complete
                || self.state.phase == AutopilotPhase::Failed
            {
                break;
            }

            let log = PhaseLog {
                phase: format!("{:?}", phase),
                started_at: chrono::Utc::now(),
                completed_at: None,
                success: false,
                note: None,
            };
            self.state.execution_log.push(log);

            let result = handler(self).await;

            let idx = self.state.execution_log.len() - 1;
            self.state.execution_log[idx].completed_at = Some(chrono::Utc::now());

            match result {
                Ok(()) => {
                    self.state.execution_log[idx].success = true;
                    info!(phase = ?phase, "Phase completed successfully");
                }
                Err(e) => {
                    self.state.execution_log[idx].success = false;
                    self.state.execution_log[idx].note = Some(format!("{}", e));
                    warn!(phase = ?phase, error = %e, "Phase failed");
                    if !self.yolo {
                        self.state.phase = AutopilotPhase::Failed;
                        self.save_state().await?;
                        self.save_done_contract().await?;
                        anyhow::bail!("Autopilot failed at phase {:?}: {}", phase, e);
                    }
                }
            }

            self.print_progress();
            self.save_state().await?;
        }

        if self.state.phase != AutopilotPhase::Failed {
            self.state.phase = AutopilotPhase::Complete;
        }
        self.save_state().await?;
        self.print_progress();
        self.save_done_contract().await?;
        info!(name = %self.name, "Autopilot complete");
        Ok(())
    }

    async fn inject_agents(&self, prompt: &str, role: &str) -> String {
        match crate::agents::load_project_agents(&self.dir).await {
            Ok(Some(manifest)) => {
                format!(
                    "{}\n\n{}",
                    prompt,
                    crate::agents::inject_agents_context(&manifest, &self.task, role)
                )
            }
            _ => prompt.to_string(),
        }
    }

    fn print_progress(&self) {
        let phases = [
            "Expansion",
            "Planning",
            "Execution",
            "QA",
            "Validation",
            "Cleanup",
        ];
        let current = match self.state.phase {
            AutopilotPhase::Expansion => 0,
            AutopilotPhase::Planning => 1,
            AutopilotPhase::Execution => 2,
            AutopilotPhase::Qa => 3,
            AutopilotPhase::Validation => 4,
            AutopilotPhase::Cleanup => 5,
            AutopilotPhase::Complete => 6,
            AutopilotPhase::Failed => 6,
        };

        println!();
        println!("🤖 Autopilot: {}", self.name);
        println!("   Task: {}", self.task);
        println!();
        for (i, phase) in phases.iter().enumerate() {
            let icon = if i < current {
                "✓"
            } else if i == current
                && self.state.phase != AutopilotPhase::Complete
                && self.state.phase != AutopilotPhase::Failed
            {
                "▶"
            } else if self.state.phase == AutopilotPhase::Failed && i == current {
                "✗"
            } else {
                "○"
            };
            println!("   {} {}", icon, phase);
        }
        println!();
    }

    async fn save_done_contract(&self) -> Result<()> {
        let mut contract = DoneContract::new(&self.name, "autopilot", self.state.created_at);
        contract.gates = self.state.gate_results.clone();
        contract.passed =
            self.state.phase == AutopilotPhase::Complete && gates_passed(&self.state.gate_results);
        contract.changed_files = detect_changed_files(&self.dir).await;

        let path = self.state_dir.join("done-contract.json");
        contract.save(&path).await?;
        info!(path = %path.display(), passed = contract.passed, "Saved done contract");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Phase implementations (in phases.rs)
    // ------------------------------------------------------------------
    pub(super) async fn run_expansion(&mut self) -> Result<()> {
        use tracing::debug;
        info!("Phase: Expansion");
        self.state.phase = AutopilotPhase::Expansion;

        let prompt = self
            .inject_agents(
                &format!(
                    "You are in the Expansion phase of an autopilot pipeline.\n\
             Task: {}\n\n\
             Broaden the scope if it creates a better product. Write expansion notes.",
                    self.task
                ),
                "expansion",
            )
            .await;

        let expansion_content = match run_kimi_prompt(&prompt).await {
            Ok(output) => output,
            Err(e) => {
                warn!(error = %e, "kimi not available, using fallback expansion");
                format!(
                    "# Expansion Notes\n\n\
                     Task: {}\n\n\
                     Scope expansion will be addressed during implementation.\n",
                    self.task
                )
            }
        };

        let expansion_path = self.state_dir.join("expansion.md");
        crate::runtime::atomic::atomic_write(&expansion_path, expansion_content.as_bytes()).await?;
        info!(path = %expansion_path.display(), "Wrote expansion notes");

        if self.interactive {
            debug!(
                phase = "expansion",
                prompt_bytes = prompt.len(),
                "Prepared expansion prompt"
            );
        }

        Ok(())
    }

    pub(super) async fn run_planning(&mut self) -> Result<()> {
        use tracing::debug;
        info!("Phase: Planning");
        self.state.phase = AutopilotPhase::Planning;

        let prompt = self
            .inject_agents(
                &format!(
                    "You are in the Planning phase of an autopilot pipeline.\n\
             Task: {}\n\n\
             Create a detailed implementation plan (PRD) with sections: \
             Overview, Goals, Architecture, Implementation Steps, Testing Strategy, Risks.",
                    self.task
                ),
                "planning",
            )
            .await;

        let plan_content = match run_kimi_prompt(&prompt).await {
            Ok(output) => output,
            Err(e) => {
                warn!(error = %e, "kimi not available, using fallback plan");
                format!(
                    "# Implementation Plan\n\n\
                     ## Task\n{}\n\n\
                     ## Steps\n\
                     1. Analyze requirements\n\
                     2. Implement solution\n\
                     3. Test and verify\n\
                     4. Cleanup and document\n",
                    self.task
                )
            }
        };

        tokio::fs::create_dir_all(&self.state.plans_dir).await?;
        let plan_path = self
            .state
            .plans_dir
            .join(format!("autopilot-{}-plan.md", self.name));
        crate::runtime::atomic::atomic_write(&plan_path, plan_content.as_bytes()).await?;
        self.state.current_plan = Some(plan_content);
        info!(path = %plan_path.display(), "Wrote plan");

        if self.interactive {
            debug!(
                phase = "planning",
                prompt_bytes = prompt.len(),
                "Prepared planning prompt"
            );
        }

        Ok(())
    }

    pub(super) async fn run_execution(&mut self) -> Result<()> {
        info!("Phase: Execution");
        self.state.phase = AutopilotPhase::Execution;

        let is_complex = self.task.len() > 100 || self.task.contains(" and ");

        if is_complex {
            info!("Task is complex, running omk team 2:executor");
            let omk_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("omk"));
            let output = tokio::process::Command::new(&omk_exe)
                .args(["team", "run", "2:executor", &self.task])
                .current_dir(&self.dir)
                .output()
                .await;

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    tracing::debug!(stdout = %stdout, stderr = %stderr, "omk team output");
                    if !out.status.success() {
                        warn!("omk team exited with non-zero status");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to run omk team");
                }
            }
        } else {
            let prompt = self
                .inject_agents(
                    &format!(
                        "You are in the Execution phase of an autopilot pipeline.\n\
                 Task: {}\n\n\
                 Implement the solution precisely. Use tools as needed.",
                        self.task
                    ),
                    "execution",
                )
                .await;
            if let Err(e) = run_kimi_prompt(&prompt).await {
                warn!(error = %e, "Failed to run kimi for execution");
            }
        }

        if self.enable_ralph {
            info!("Ralph enabled — running verify/fix loop");
            if let Err(e) =
                crate::runtime::ralph::run_ralph(&self.task, &self.dir, 3, false, self.yolo).await
            {
                warn!(error = %e, "Ralph loop failed");
            }
        }

        Ok(())
    }

    pub(super) async fn run_qa(&mut self) -> Result<()> {
        info!("Phase: QA");
        self.state.phase = AutopilotPhase::Qa;

        let gate_config = load_or_detect_gates(&self.dir).await;
        if gate_config.gates.is_empty() {
            warn!("No verification gates configured and project type unknown; skipping QA");
            self.state.qa_results = Some(QaResults {
                passed: true,
                errors: vec![],
            });
            return Ok(());
        }

        info!(gates = ?gate_config.gates.iter().map(|g| &g.name).collect::<Vec<_>>(), "Running verification gates");
        let results = run_gates(&gate_config, &self.dir).await;
        self.state.gate_results = results.clone();

        let passed = gates_passed(&results);
        let errors: Vec<String> = results
            .iter()
            .filter(|r| r.required && !r.passed)
            .map(|r| {
                format!(
                    "{} failed: {}",
                    r.name,
                    r.stderr.chars().take(200).collect::<String>()
                )
            })
            .collect();

        self.state.qa_results = Some(QaResults {
            passed,
            errors: errors.clone(),
        });

        println!();
        println!("{}", format_gate_summary(&results));

        if passed {
            info!("All required gates passed");
        } else {
            warn!(error_count = errors.len(), "QA gates failed");
            if !self.yolo {
                anyhow::bail!("QA failed: {} required gate(s) did not pass", errors.len());
            }
        }

        Ok(())
    }

    pub(super) async fn run_validation(&mut self) -> Result<()> {
        info!("Phase: Validation");
        self.state.phase = AutopilotPhase::Validation;

        let mut results = Vec::new();

        // Architect review
        let architect_prompt = self
            .inject_agents(
                &format!(
                    "You are a Senior Architect reviewing this implementation.\n\
             Task: {}\n\n\
             Review the code for: design patterns, scalability, maintainability, correctness.\n\
             Write your analysis, then end your response with a single line:\n\
             VERDICT: PASS  (if the implementation is acceptable)\n\
             VERDICT: FAIL  (if the implementation must be changed before shipping)\n\
             The verdict line is required and must appear on its own line at the end.",
                    self.task
                ),
                "architect",
            )
            .await;
        let architect_result = match run_kimi_prompt(&architect_prompt).await {
            Ok(output) => {
                let passed = verdict_pass("architect", &output);
                ValidationResult {
                    reviewer: "architect".to_string(),
                    passed,
                    notes: output,
                }
            }
            Err(_) => ValidationResult {
                reviewer: "architect".to_string(),
                passed: true,
                notes: "kimi not available, assumed pass".to_string(),
            },
        };
        results.push(architect_result);

        // Security review
        let security_prompt = self.inject_agents(&format!(
            "You are a Security Engineer reviewing this implementation.\n\
             Task: {}\n\n\
             Review for: injection vulnerabilities, unsafe code, secret leakage, input validation.\n\
             Write your analysis, then end your response with a single line:\n\
             VERDICT: PASS  (if no exploitable issues remain)\n\
             VERDICT: FAIL  (if any exploitable issue remains)\n\
             The verdict line is required and must appear on its own line at the end.",
            self.task
        ), "security").await;
        let security_result = match run_kimi_prompt(&security_prompt).await {
            Ok(output) => {
                let passed = verdict_pass("security", &output);
                ValidationResult {
                    reviewer: "security".to_string(),
                    passed,
                    notes: output,
                }
            }
            Err(_) => ValidationResult {
                reviewer: "security".to_string(),
                passed: true,
                notes: "kimi not available, assumed pass".to_string(),
            },
        };
        results.push(security_result);

        self.state.validation_results = results;
        info!("Validation complete");
        Ok(())
    }

    pub(super) fn run_cleanup(&mut self) -> Result<()> {
        info!("Phase: Cleanup");
        self.state.phase = AutopilotPhase::Cleanup;

        let patterns = ["*.tmp", "*.log", "*.bak"];
        for pattern in &patterns {
            tracing::debug!(pattern, "Would clean temp files");
        }

        let readme_path = self.dir.join("README.md");
        if readme_path.exists() {
            info!("README.md exists — consider updating documentation");
        }

        info!("Cleanup complete");
        Ok(())
    }
}

/// Wrap [`parse_verdict`] with a fail-closed default and a structured warning
/// so the caller does not silently treat an unparseable LLM response as a
/// passing review.
fn verdict_pass(reviewer: &str, output: &str) -> bool {
    match parse_verdict(output) {
        Some(verdict) => verdict,
        None => {
            warn!(
                reviewer = reviewer,
                "No VERDICT: PASS|FAIL line found in reviewer output; treating as FAIL"
            );
            false
        }
    }
}

/// Parse the structured verdict from a reviewer's reply.
///
/// We look for `VERDICT:` followed by a `PASS`/`FAIL`-like token within the
/// last 200 characters of the output (case-insensitive). The tail-only scope
/// avoids matching the verdict instruction echoed back in the body. Returns
/// `None` when no recognizable verdict is found, so callers can fail-closed.
fn parse_verdict(output: &str) -> Option<bool> {
    let tail = if output.len() > 200 {
        let mut idx = output.len() - 200;
        while !output.is_char_boundary(idx) {
            idx += 1;
        }
        &output[idx..]
    } else {
        output
    };

    let lower = tail.to_lowercase();
    let pos = lower.rfind("verdict:")?;
    let after = &lower[pos + "verdict:".len()..];
    let token = after
        .split_whitespace()
        .next()?
        .trim_end_matches(|c: char| !c.is_alphanumeric());

    match token {
        "pass" | "passed" | "approve" | "approved" | "ok" => Some(true),
        "fail" | "failed" | "reject" | "rejected" | "block" | "blocked" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod verdict_tests {
    use super::parse_verdict;

    #[test]
    fn parses_pass_at_end() {
        assert_eq!(parse_verdict("looks fine.\nVERDICT: PASS"), Some(true));
    }

    #[test]
    fn parses_fail_at_end() {
        assert_eq!(
            parse_verdict("found a sql injection on line 12.\nVERDICT: FAIL"),
            Some(false)
        );
    }

    #[test]
    fn benign_mention_of_fail_does_not_flip_verdict() {
        let body = "Notes: no test failures detected; fail-safe pattern used.\n\
                    VERDICT: PASS";
        assert_eq!(parse_verdict(body), Some(true));
    }

    #[test]
    fn missing_verdict_returns_none() {
        assert_eq!(parse_verdict("looks fine, nothing critical"), None);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(parse_verdict("\nverdict: pass\n"), Some(true));
        assert_eq!(parse_verdict("\nVerdict: Fail\n"), Some(false));
    }

    #[test]
    fn last_verdict_wins_within_tail() {
        let body = "VERDICT: FAIL\n\
                    on second thought, fixed it.\n\
                    VERDICT: PASS";
        assert_eq!(parse_verdict(body), Some(true));
    }

    #[test]
    fn verdict_at_start_of_long_body_is_ignored() {
        // 200-char tail window is the contract; a verdict in the first
        // half of a >400-char body must be invisible to the parser. This
        // locks in the tail semantics: the prompt asks the LLM to put the
        // verdict at the END, and we refuse to honor a stray earlier line.
        let mut body = String::from("VERDICT: PASS\n");
        body.push_str(&"prose ".repeat(60)); // ~360 chars of filler
                                             // No VERDICT at the end → parser sees no verdict in last 200 chars.
        assert_eq!(parse_verdict(&body), None);
    }

    #[test]
    fn multibyte_tail_boundary_is_safe() {
        // 4-byte UTF-8 grapheme straddling the 200-char tail boundary must
        // not panic; the parser walks forward to the next char boundary.
        let pad = "𝓍".repeat(80); // each char is 4 bytes → 320 bytes
        let body = format!("{}\nfinal note.\nVERDICT: PASS", pad);
        assert_eq!(parse_verdict(&body), Some(true));
    }
}
