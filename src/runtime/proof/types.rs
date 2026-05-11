use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::runtime::events::RunId;

/// Final readiness report for a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub run_id: RunId,
    pub status: ProofStatus,
    pub generated_at: DateTime<Utc>,
    pub changed_files: Vec<ChangedFile>,
    pub gates: Vec<ProofGate>,
    pub failures: Vec<ProofFailure>,
    pub retries: Vec<ProofRetry>,
    pub known_gaps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_evidence: Option<WireEvidenceSummary>,
    pub summary: String,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WireEvidenceSummary {
    pub event_count: usize,
    pub request_count: usize,
    pub output_count: usize,
    pub prompt_like_messages: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique_methods: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique_events: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique_requests: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofStatus {
    Ready,
    NotReady,
    Failed,
}

impl std::fmt::Display for ProofStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofStatus::Ready => write!(f, "ready"),
            ProofStatus::NotReady => write!(f, "not_ready"),
            ProofStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub operation: String, // "created", "modified", "deleted"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofGate {
    pub name: String,
    pub status: GateStatus,
    pub required: bool,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofFailure {
    pub task_id: Option<String>,
    pub worker_id: Option<String>,
    pub description: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRetry {
    pub task_id: String,
    pub attempt: u32,
    pub reason: String,
}

impl Proof {
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            status: ProofStatus::NotReady,
            generated_at: Utc::now(),
            changed_files: Vec::new(),
            gates: Vec::new(),
            failures: Vec::new(),
            retries: Vec::new(),
            known_gaps: Vec::new(),
            wire_evidence: None,
            summary: String::new(),
            elapsed_secs: 0,
        }
    }

    pub fn proof_path(state_dir: &Path) -> PathBuf {
        state_dir.join("proof.json")
    }

    pub fn write_json(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub async fn save(&self, state_dir: &Path) -> anyhow::Result<()> {
        let path = Self::proof_path(state_dir);
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
        info!(path = %path.display(), status = %self.status, "Saved proof");
        Ok(())
    }

    pub async fn load(state_dir: &Path) -> anyhow::Result<Option<Self>> {
        let path = Self::proof_path(state_dir);
        if !path.exists() {
            return Ok(None);
        }
        let json = tokio::fs::read_to_string(&path).await?;
        let proof: Self = serde_json::from_str(&json)?;
        Ok(Some(proof))
    }

    pub fn readiness(&self) -> &'static str {
        match self.status {
            ProofStatus::Ready => "ready_for_handoff",
            ProofStatus::NotReady => "needs_follow_up",
            ProofStatus::Failed => "blocked",
        }
    }

    pub fn readiness_text(&self) -> &'static str {
        match self.status {
            ProofStatus::Ready => {
                "Ready for handoff: required gates passed and no blocking failures."
            }
            ProofStatus::NotReady => "Needs follow-up: required gates are incomplete or missing.",
            ProofStatus::Failed => "Blocked: failures or required gate failures must be resolved.",
        }
    }

    pub fn gate_counts(&self) -> (usize, usize, usize) {
        (
            self.gates
                .iter()
                .filter(|g| g.status == GateStatus::Passed)
                .count(),
            self.gates
                .iter()
                .filter(|g| g.status == GateStatus::Failed)
                .count(),
            self.gates
                .iter()
                .filter(|g| g.status == GateStatus::Skipped)
                .count(),
        )
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let (gates_passed, gates_failed, gates_skipped) = self.gate_counts();

        md.push_str(&format!("# Proof Report for {}\n\n", self.run_id));
        md.push_str(&format!("**Status:** {}  \n", self.status));
        md.push_str(&format!("**Readiness:** {}  \n", self.readiness()));
        md.push_str(&format!(
            "**Readiness Detail:** {}  \n",
            self.readiness_text()
        ));
        md.push_str(&format!("**Generated:** {}  \n", self.generated_at));
        if self.elapsed_secs > 0 {
            md.push_str(&format!("**Duration:** {}s  \n", self.elapsed_secs));
        }
        md.push('\n');

        md.push_str("## Verdict\n\n");
        md.push_str(&format!("- status: `{}`\n", self.status));
        md.push_str(&format!("- readiness: `{}`\n", self.readiness()));
        md.push_str(&format!(
            "- changed_files: `{}`\n",
            self.changed_files.len()
        ));
        md.push_str(&format!("- gates_total: `{}`\n", self.gates.len()));
        md.push_str(&format!(
            "- gates: passed=`{}`, failed=`{}`, skipped=`{}`\n",
            gates_passed, gates_failed, gates_skipped
        ));
        md.push_str(&format!("- failures: `{}`\n", self.failures.len()));
        md.push_str(&format!("- retries: `{}`\n", self.retries.len()));
        md.push_str(&format!("- known_gaps: `{}`\n\n", self.known_gaps.len()));

        md.push_str("## Wire Evidence\n\n");
        if let Some(wire) = &self.wire_evidence {
            md.push_str(&format!(
                "- events: `{}`\n- requests: `{}`\n- outputs: `{}`\n- prompt_like_messages: `{}`\n",
                wire.event_count, wire.request_count, wire.output_count, wire.prompt_like_messages
            ));
            if !wire.unique_methods.is_empty() {
                md.push_str(&format!(
                    "- methods: `{}`\n",
                    wire.unique_methods.join(", ")
                ));
            }
            if !wire.unique_events.is_empty() {
                md.push_str(&format!(
                    "- wire_events: `{}`\n",
                    wire.unique_events.join(", ")
                ));
            }
            if !wire.unique_requests.is_empty() {
                md.push_str(&format!(
                    "- wire_requests: `{}`\n",
                    wire.unique_requests.join(", ")
                ));
            }
            md.push('\n');
        } else {
            md.push_str("_none_\n\n");
        }

        md.push_str("## Changed Files\n\n");
        if self.changed_files.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            md.push_str("| Operation | Path |\n");
            md.push_str("|-----------|------|\n");
            for f in &self.changed_files {
                md.push_str(&format!("| {} | {} |\n", f.operation, f.path));
            }
            md.push('\n');
        }

        md.push_str("## Gates\n\n");
        if self.gates.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            md.push_str("| Gate | Status | Required |\n");
            md.push_str("|------|--------|----------|\n");
            for g in &self.gates {
                let status_str = match g.status {
                    GateStatus::Passed => "passed",
                    GateStatus::Failed => "failed",
                    GateStatus::Skipped => "skipped",
                };
                let required_str = if g.required { "Yes" } else { "No" };
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    g.name, status_str, required_str
                ));
            }
            md.push('\n');
        }

        md.push_str("## Failures\n\n");
        if self.failures.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            for f in &self.failures {
                md.push_str(&format!(
                    "- **{}** — {}\n",
                    f.worker_id.as_deref().unwrap_or("?"),
                    f.description
                ));
            }
            md.push('\n');
        }

        md.push_str("## Retries\n\n");
        if self.retries.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            for r in &self.retries {
                md.push_str(&format!(
                    "- **{}** (attempt {}): {}\n",
                    r.task_id, r.attempt, r.reason
                ));
            }
            md.push('\n');
        }

        md.push_str("## Known Gaps\n\n");
        if self.known_gaps.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            for g in &self.known_gaps {
                md.push_str(&format!("- {}\n", g));
            }
            md.push('\n');
        }

        md.push_str("## Summary\n\n");
        md.push_str(&format!("{}\n\n", self.summary));
        md.push_str("---\n\n");
        md.push_str(&format!("Readiness verdict: `{}`.\n", self.readiness()));
        md.push_str(&format!("{}\n", self.readiness_text()));

        md
    }
}
