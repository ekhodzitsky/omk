use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::runtime::autopilot::types::{AutopilotPhase, AutopilotState, PhaseLog};
use crate::runtime::gates::{detect_changed_files, gates_passed, DoneContract};

/// Autopilot engine that drives the 6-phase pipeline.
#[derive(Debug)]
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
}

mod phases;
mod verdict;
