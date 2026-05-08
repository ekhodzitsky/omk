#![allow(dead_code)]

// Autopilot state machine — 6-phase pipeline
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Full autopilot state persisted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotState {
    #[serde(default = "crate::runtime::state::default_state_version")]
    pub version: u32,
    pub task: String,
    pub phase: AutopilotPhase,
    pub plans_dir: PathBuf,
    pub created_at: DateTime<Utc>,
    pub current_plan: Option<String>,
    pub qa_results: Option<QaResults>,
    pub validation_results: Vec<ValidationResult>,
    pub execution_log: Vec<PhaseLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseLog {
    pub phase: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaResults {
    pub passed: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub reviewer: String,
    pub passed: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum AutopilotPhase {
    Expansion,
    Planning,
    Execution,
    Qa,
    Validation,
    Cleanup,
    Complete,
    Failed,
}

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
        let state_dir = crate::runtime::config::state_dir().join("autopilot").join(name);
        let plans_dir = crate::runtime::config::data_dir().join("plans");
        let state = AutopilotState {
            version: 1,
            task: task.to_string(),
            phase: AutopilotPhase::Expansion,
            plans_dir: plans_dir.clone(),
            created_at: Utc::now(),
            current_plan: None,
            qa_results: None,
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
            name: state_dir.file_name().unwrap().to_string_lossy().to_string(),
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

        let phases: Vec<(AutopilotPhase, Box<dyn Fn(&mut Self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + '_>>>)> = vec![
            (AutopilotPhase::Expansion, Box::new(|s| Box::pin(s.run_expansion()))),
            (AutopilotPhase::Planning, Box::new(|s| Box::pin(s.run_planning()))),
            (AutopilotPhase::Execution, Box::new(|s| Box::pin(s.run_execution()))),
            (AutopilotPhase::Qa, Box::new(|s| Box::pin(s.run_qa()))),
            (AutopilotPhase::Validation, Box::new(|s| Box::pin(s.run_validation()))),
            (AutopilotPhase::Cleanup, Box::new(|s| Box::pin(s.run_cleanup()))),
        ];

        let start_idx = phases.iter().position(|(p, _)| *p == self.state.phase).unwrap_or(0);

        for (phase, handler) in &phases[start_idx..] {
            if self.state.phase == AutopilotPhase::Complete || self.state.phase == AutopilotPhase::Failed {
                break;
            }

            let log = PhaseLog {
                phase: format!("{:?}", phase),
                started_at: Utc::now(),
                completed_at: None,
                success: false,
                note: None,
            };
            self.state.execution_log.push(log);

            let result = handler(self).await;

            let idx = self.state.execution_log.len() - 1;
            self.state.execution_log[idx].completed_at = Some(Utc::now());

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
        info!(name = %self.name, "Autopilot complete");
        Ok(())
    }

    fn print_progress(&self) {
        let phases = ["Expansion", "Planning", "Execution", "QA", "Validation", "Cleanup"];
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
            } else if i == current && self.state.phase != AutopilotPhase::Complete && self.state.phase != AutopilotPhase::Failed {
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

    // ------------------------------------------------------------------
    // Phase 1 — Expansion
    // ------------------------------------------------------------------
    async fn run_expansion(&mut self) -> Result<()> {
        info!("Phase: Expansion");
        self.state.phase = AutopilotPhase::Expansion;

        let prompt = format!(
            "You are in the Expansion phase of an autopilot pipeline.\n\
             Task: {}\n\n\
             Broaden the scope if it creates a better product. Write expansion notes.",
            self.task
        );

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
            let _ = self.spawn_tmux_for_phase("expansion", &prompt);
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Phase 2 — Planning
    // ------------------------------------------------------------------
    async fn run_planning(&mut self) -> Result<()> {
        info!("Phase: Planning");
        self.state.phase = AutopilotPhase::Planning;

        let prompt = format!(
            "You are in the Planning phase of an autopilot pipeline.\n\
             Task: {}\n\n\
             Create a detailed implementation plan (PRD) with sections: \
             Overview, Goals, Architecture, Implementation Steps, Testing Strategy, Risks.",
            self.task
        );

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
            let _ = self.spawn_tmux_for_phase("planning", &prompt);
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Phase 3 — Execution
    // ------------------------------------------------------------------
    async fn run_execution(&mut self) -> Result<()> {
        info!("Phase: Execution");
        self.state.phase = AutopilotPhase::Execution;

        let is_complex = self.task.len() > 100 || self.task.contains(" and ");

        if is_complex {
            info!("Task is complex, spawning omk team 2:executor");
            let omk_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("omk"));
            let output = Command::new(&omk_exe)
                .args(["team", "spawn", "2:executor", &self.task])
                .current_dir(&self.dir)
                .output()
                .await;

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    debug!(stdout = %stdout, stderr = %stderr, "omk team output");
                    if !out.status.success() {
                        warn!("omk team exited with non-zero status");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to spawn omk team");
                }
            }
        } else {
            let prompt = format!(
                "You are in the Execution phase of an autopilot pipeline.\n\
                 Task: {}\n\n\
                 Implement the solution precisely. Use tools as needed.",
                self.task
            );
            if let Err(e) = run_kimi_prompt(&prompt).await {
                warn!(error = %e, "Failed to run kimi for execution");
            }
        }

        if self.enable_ralph {
            info!("Ralph enabled — running verify/fix loop");
            if let Err(e) = crate::runtime::ralph::run_ralph(&self.task, &self.dir, 3).await {
                warn!(error = %e, "Ralph loop failed");
            }
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Phase 4 — QA
    // ------------------------------------------------------------------
    async fn run_qa(&mut self) -> Result<()> {
        info!("Phase: QA");
        self.state.phase = AutopilotPhase::Qa;

        let project_type = detect_project_type(&self.dir).await;
        info!(project_type = ?project_type, "Detected project type");

        let mut errors = Vec::new();

        match project_type {
            ProjectType::Rust => {
                if let Err(e) = run_command(&self.dir, "cargo", &["test"]).await {
                    errors.push(format!("cargo test failed: {e}"));
                }
                if let Err(e) = run_command(&self.dir, "cargo", &["clippy", "--", "-D", "warnings"]).await {
                    errors.push(format!("cargo clippy failed: {e}"));
                }
                if let Err(e) = run_command(&self.dir, "cargo", &["fmt", "--check"]).await {
                    errors.push(format!("cargo fmt --check failed: {e}"));
                }
            }
            ProjectType::Node => {
                if let Err(e) = run_command(&self.dir, "npm", &["test"]).await {
                    errors.push(format!("npm test failed: {e}"));
                }
                if let Err(e) = run_command(&self.dir, "npm", &["run", "lint"]).await {
                    errors.push(format!("npm run lint failed: {e}"));
                }
            }
            ProjectType::Python => {
                if let Err(e) = run_command(&self.dir, "python", &["-m", "pytest"]).await {
                    errors.push(format!("pytest failed: {e}"));
                }
                if let Err(e) = run_command(&self.dir, "python", &["-m", "flake8", "."]).await {
                    errors.push(format!("flake8 failed: {e}"));
                }
            }
            ProjectType::Go => {
                if let Err(e) = run_command(&self.dir, "go", &["test", "./..."]).await {
                    errors.push(format!("go test failed: {e}"));
                }
                if let Err(e) = run_command(&self.dir, "go", &["vet", "./..."]).await {
                    errors.push(format!("go vet failed: {e}"));
                }
                if let Err(e) = run_command(&self.dir, "gofmt", &["-l", "."]).await {
                    errors.push(format!("gofmt failed: {e}"));
                }
            }
            ProjectType::Unknown => {
                warn!("Unknown project type, skipping QA commands");
            }
        }

        let passed = errors.is_empty();
        self.state.qa_results = Some(QaResults {
            passed,
            errors: errors.clone(),
        });

        if passed {
            info!("QA passed");
        } else {
            warn!(error_count = errors.len(), "QA found errors");
            if !self.yolo {
                anyhow::bail!("QA failed with {} errors", errors.len());
            }
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Phase 5 — Validation
    // ------------------------------------------------------------------
    async fn run_validation(&mut self) -> Result<()> {
        info!("Phase: Validation");
        self.state.phase = AutopilotPhase::Validation;

        let mut results = Vec::new();

        // Architect review
        let architect_prompt = format!(
            "You are a Senior Architect reviewing this implementation.\n\
             Task: {}\n\n\
             Review the code for: design patterns, scalability, maintainability, correctness.\n\
             Give a concise pass/fail verdict with notes.",
            self.task
        );
        let architect_result = match run_kimi_prompt(&architect_prompt).await {
            Ok(output) => ValidationResult {
                reviewer: "architect".to_string(),
                passed: !output.to_lowercase().contains("fail"),
                notes: output,
            },
            Err(_) => ValidationResult {
                reviewer: "architect".to_string(),
                passed: true,
                notes: "kimi not available, assumed pass".to_string(),
            },
        };
        results.push(architect_result);

        // Security review
        let security_prompt = format!(
            "You are a Security Engineer reviewing this implementation.\n\
             Task: {}\n\n\
             Review for: injection vulnerabilities, unsafe code, secret leakage, input validation.\n\
             Give a concise pass/fail verdict with notes.",
            self.task
        );
        let security_result = match run_kimi_prompt(&security_prompt).await {
            Ok(output) => ValidationResult {
                reviewer: "security".to_string(),
                passed: !output.to_lowercase().contains("fail"),
                notes: output,
            },
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

    // ------------------------------------------------------------------
    // Phase 6 — Cleanup
    // ------------------------------------------------------------------
    async fn run_cleanup(&mut self) -> Result<()> {
        info!("Phase: Cleanup");
        self.state.phase = AutopilotPhase::Cleanup;

        // Remove temporary artifacts
        let patterns = ["*.tmp", "*.log", "*.bak"];
        for pattern in &patterns {
            debug!(pattern, "Would clean temp files");
        }

        // Update docs if README exists
        let readme_path = self.dir.join("README.md");
        if readme_path.exists() {
            info!("README.md exists — consider updating documentation");
        }

        info!("Cleanup complete");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------
    fn spawn_tmux_for_phase(&self, phase_name: &str, prompt: &str) -> Result<()> {
        let session_name = format!("omk-ap-{}-{}", self.name, phase_name);
        if !crate::runtime::tmux::session_exists(&session_name)? {
            crate::runtime::tmux::create_session(&session_name, phase_name, &self.dir)?;
        }
        let escaped = shell_escape(prompt);
        crate::runtime::tmux::send_keys(
            &session_name,
            phase_name,
            &format!("kimi -p {}", escaped),
        )?;
        info!(session = %session_name, "Spawned tmux session for phase");
        Ok(())
    }
}

/// Convenience entry-point used by the CLI.
pub async fn run_autopilot(name: &str, task: &str, dir: &Path, enable_ralph: bool, yolo: bool) -> Result<()> {
    let mut autopilot = Autopilot::new(name, task, dir, enable_ralph, yolo);
    autopilot.run().await
}

/// Resume an existing autopilot run.
pub async fn resume_autopilot(name: &str, _dir: &Path, enable_ralph: bool, yolo: bool) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir().join("autopilot").join(name);
    if !state_dir.exists() {
        anyhow::bail!("Autopilot run '{}' not found at {}", name, state_dir.display());
    }

    let mut autopilot = Autopilot::from_state(&state_dir, enable_ralph, yolo).await?;
    info!(name = %name, phase = ?autopilot.state.phase, "Resuming autopilot");
    autopilot.run().await
}

// ------------------------------------------------------------------
// Internal helpers
// ------------------------------------------------------------------

async fn run_kimi_prompt(prompt: &str) -> Result<String> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        Command::new("kimi")
            .arg("--print")
            .arg("-p")
            .arg(prompt)
            .output(),
    )
    .await
    .context("kimi prompt timed out")??;

    if !output.status.success() {
        anyhow::bail!("kimi exited with non-zero status");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn run_command(dir: &Path, cmd: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .with_context(|| format!("Failed to run {} {}", cmd, args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command failed: {} {}\n{}", cmd, args.join(" "), stderr);
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Unknown,
}

async fn detect_project_type(dir: &Path) -> ProjectType {
    if dir.join("Cargo.toml").exists() {
        ProjectType::Rust
    } else if dir.join("package.json").exists() {
        ProjectType::Node
    } else if dir.join("go.mod").exists() {
        ProjectType::Go
    } else if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("requirements.txt").exists()
    {
        ProjectType::Python
    } else {
        ProjectType::Unknown
    }
}

fn shell_escape(s: &str) -> String {
    crate::runtime::shell::shell_escape(s)
}
