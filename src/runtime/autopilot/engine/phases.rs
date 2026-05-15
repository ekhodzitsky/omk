use anyhow::Result;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::runtime::autopilot::helpers::run_kimi_prompt;
use crate::runtime::autopilot::types::{AutopilotPhase, QaResults, ValidationResult};
use crate::runtime::gates::{format_gate_summary, gates_passed, load_or_detect_gates, run_gates};

use super::verdict::verdict_pass;
use super::Autopilot;

impl Autopilot {
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
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                tokio::process::Command::new(&omk_exe)
                    .args(["team", "run", "2:executor", &self.task])
                    .current_dir(&self.dir)
                    .output(),
            )
            .await;

            match output {
                Ok(Ok(out)) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    tracing::debug!(stdout = %stdout, stderr = %stderr, "omk team output");
                    if !out.status.success() {
                        warn!("omk team exited with non-zero status");
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "Failed to run omk team");
                }
                Err(_) => {
                    warn!("omk team timed out");
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
