use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::state::{GoalPhase, GoalState, GoalStatus, GOAL_PROOF_FILE};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalProgressSnapshot {
    pub phase: GoalPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task: Option<String>,
    #[serde(default)]
    pub done: Vec<String>,
    #[serde(default)]
    pub next: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub gates: Vec<String>,
    #[serde(default)]
    pub reviews: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_path: Option<PathBuf>,
    #[serde(default)]
    pub narrative: Vec<GoalProgressLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalProgressLine {
    pub kind: GoalProgressLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalProgressLineKind {
    Implemented,
    RunningVerification,
    ReviewBlocker,
    Next,
    Blocked,
    Ready,
    Decision,
}

impl GoalProgressSnapshot {
    pub fn new(phase: GoalPhase) -> Self {
        Self {
            phase,
            current_task: None,
            done: Vec::new(),
            next: Vec::new(),
            blockers: Vec::new(),
            gates: Vec::new(),
            reviews: Vec::new(),
            proof_path: None,
            narrative: Vec::new(),
        }
    }

    pub fn from_goal_state(state: &GoalState) -> Self {
        let phase_text = progress_text_for_phase(state.phase);
        let mut snapshot = Self::new(state.phase)
            .current_task(phase_text.current_task)
            .proof_path(goal_proof_path(state));

        snapshot = match state.status {
            GoalStatus::Running => snapshot.next_step(phase_text.next_step),
            GoalStatus::Ready => snapshot.ready("proof-backed goal complete"),
            GoalStatus::NotReady => snapshot.blocked("proof is not ready"),
            GoalStatus::BlockedOnHuman => snapshot.blocked(goal_blocker_text(
                state,
                "human decision required before goal can continue",
            )),
            GoalStatus::BlockedOnExternal => snapshot.blocked(goal_blocker_text(
                state,
                "external dependency required before goal can continue",
            )),
            GoalStatus::NeedsMoreBudget => snapshot.blocked(goal_blocker_text(
                state,
                "budget exhausted before readiness",
            )),
            GoalStatus::FailedInfra => {
                snapshot.blocked(goal_blocker_text(state, "infrastructure failure"))
            }
            GoalStatus::Paused => snapshot.blocked("operator paused goal"),
            GoalStatus::Cancelled => snapshot.blocked(goal_blocker_text(state, "goal cancelled")),
        };

        snapshot
    }

    pub fn current_task(mut self, task: impl Into<String>) -> Self {
        if let Some(task) = clean_text(task) {
            self.current_task = Some(task);
        }
        self
    }

    pub fn proof_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if !path.as_os_str().is_empty() {
            self.proof_path = Some(path);
        }
        self
    }

    pub fn implemented(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            push_unique(&mut self.done, text.clone());
            self.push_line(GoalProgressLineKind::Implemented, text);
        }
        self
    }

    pub fn running_verification(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            push_unique(&mut self.gates, text.clone());
            self.push_line(GoalProgressLineKind::RunningVerification, text);
        }
        self
    }

    pub fn review_blocker(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            push_unique(&mut self.reviews, text.clone());
            push_unique(&mut self.blockers, text.clone());
            self.push_line(GoalProgressLineKind::ReviewBlocker, text);
        }
        self
    }

    pub fn next_step(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            push_unique(&mut self.next, text.clone());
            self.push_line(GoalProgressLineKind::Next, text);
        }
        self
    }

    pub fn blocked(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            push_unique(&mut self.blockers, text.clone());
            self.push_line(GoalProgressLineKind::Blocked, text);
        }
        self
    }

    pub fn ready(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            self.push_line(GoalProgressLineKind::Ready, text);
        }
        self
    }

    pub fn decision(mut self, text: impl Into<String>) -> Self {
        if let Some(text) = clean_text(text) {
            self.push_line(GoalProgressLineKind::Decision, text);
        }
        self
    }

    fn push_line(&mut self, kind: GoalProgressLineKind, text: String) {
        self.narrative.push(GoalProgressLine { kind, text });
    }
}

impl GoalProgressLine {
    pub fn render(&self) -> String {
        match self.kind {
            GoalProgressLineKind::Implemented => format!("implemented {}", self.text),
            GoalProgressLineKind::RunningVerification => {
                format!("running verification {}", self.text)
            }
            GoalProgressLineKind::ReviewBlocker => {
                format!("review found blocker {}, creating fix task", self.text)
            }
            GoalProgressLineKind::Next => format!("next: {}", self.text),
            GoalProgressLineKind::Blocked => format!("blocked: {}", self.text),
            GoalProgressLineKind::Ready => format!("ready: {}", self.text),
            GoalProgressLineKind::Decision => format!("considering {}", self.text),
        }
    }
}

fn goal_proof_path(state: &GoalState) -> PathBuf {
    state
        .artifacts
        .iter()
        .rev()
        .find(|artifact| artifact.kind == "proof")
        .map(|artifact| resolve_goal_path(&state.state_dir, &artifact.path))
        .unwrap_or_else(|| resolve_goal_path(&state.state_dir, Path::new(GOAL_PROOF_FILE)))
}

fn resolve_goal_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() || base.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

struct GoalPhaseProgressText {
    current_task: &'static str,
    next_step: &'static str,
}

fn progress_text_for_phase(phase: GoalPhase) -> GoalPhaseProgressText {
    match phase {
        GoalPhase::Intake => GoalPhaseProgressText {
            current_task: "goal-intake",
            next_step: "classify goal and define oracle",
        },
        GoalPhase::Planning => GoalPhaseProgressText {
            current_task: "goal-planning",
            next_step: "write PRD, technical plan, and test spec",
        },
        GoalPhase::Decomposition => GoalPhaseProgressText {
            current_task: "goal-decomposition",
            next_step: "split work into PR-sized slices",
        },
        GoalPhase::Execution => GoalPhaseProgressText {
            current_task: "goal-execution",
            next_step: "dispatch bounded goal tasks",
        },
        GoalPhase::VerificationDesign => GoalPhaseProgressText {
            current_task: "goal-verification-design",
            next_step: "run verification wall",
        },
        GoalPhase::Proof => GoalPhaseProgressText {
            current_task: "goal-proof",
            next_step: "refresh proof bundle",
        },
    }
}

fn goal_blocker_text(state: &GoalState, fallback: &'static str) -> String {
    state
        .failure
        .as_ref()
        .map(|failure| failure.reason.trim())
        .filter(|reason| !reason.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

fn clean_text(text: impl Into<String>) -> Option<String> {
    let text = text.into();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
