use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::runtime::config::{HEARTBEAT_FILE, INBOX_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, GateId, RunId, TaskId, WorkerId,
};
use crate::runtime::gates::{
    detect_changed_files, gates_passed, load_or_detect_gates, run_gates_with_evidence, GateResult,
};
use crate::runtime::scheduler::runner::{RunSummary, TeamRunner};
use crate::runtime::scheduler::task::Task;
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::{WorkerResult, WorkerSpec};

pub const GOALS_DIR: &str = "goals";
pub const GOAL_STATE_FILE: &str = "goal.json";
pub const GOAL_FAILURE_FILE: &str = "failure.json";
pub const GOAL_PRD_FILE: &str = "prd.md";
pub const GOAL_TECHNICAL_PLAN_FILE: &str = "technical-plan.md";
pub const GOAL_TEST_SPEC_FILE: &str = "test-spec.md";
pub const GOAL_TASK_GRAPH_FILE: &str = "task-graph.json";
pub const GOAL_PROOF_FILE: &str = "proof.json";
pub const GOAL_ARTIFACTS_DIR: &str = "artifacts";
pub const GOAL_GATE_ARTIFACTS_DIR: &str = "gates";
pub const GOAL_AGENT_RUNS_DIR: &str = "agent-runs";
const GOAL_CONTROLLER_ACTOR: &str = "goal-controller";
const GOAL_LOCAL_VERIFY_TASK_ID: &str = "goal-local-verify";
const GOAL_AGENT_EXECUTE_TASK_ID: &str = "goal-agent-execute";
const GOAL_AGENT_IMPLEMENT_TASK_ID: &str = "goal-agent-implement";
const GOAL_AGENT_VERIFY_TASK_ID: &str = "goal-agent-verify";
const GOAL_AGENT_PUBLISH_TASK_ID: &str = "goal-agent-publish-crates-io";
const GOAL_AGENT_TASK_POLICY_FILE: &str = "task-policy.json";
const GOAL_AGENT_TASK_PROPOSALS_FILE: &str = "agent-task-proposals.json";
const GOAL_AGENT_TASK_PROPOSAL_MARKER: &str = "OMK_TASK_PROPOSAL:";
const GOAL_REVIEW_TASK_ID: &str = "goal-review";
const GOAL_SECURITY_REVIEW_TASK_ID: &str = "goal-security-review";
const GOAL_AGENT_WORKER_ID: &str = "goal-agent-worker-0";
const GOAL_AGENT_WORKER_ROLE: &str = "executor";
const GOAL_REVIEW_ARTIFACTS_DIR: &str = "reviews";
const GOAL_REVIEW_FILE: &str = "goal-review.md";
const GOAL_SECURITY_REVIEW_FILE: &str = "goal-security-review.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Running,
    Ready,
    NotReady,
    BlockedOnHuman,
    BlockedOnExternal,
    NeedsMoreBudget,
    FailedInfra,
    Cancelled,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalStatus::Running => "running",
            GoalStatus::Ready => "ready",
            GoalStatus::NotReady => "not_ready",
            GoalStatus::BlockedOnHuman => "blocked_on_human",
            GoalStatus::BlockedOnExternal => "blocked_on_external",
            GoalStatus::NeedsMoreBudget => "needs_more_budget",
            GoalStatus::FailedInfra => "failed_infra",
            GoalStatus::Cancelled => "cancelled",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalPhase {
    Intake,
    Planning,
    Decomposition,
    Execution,
    VerificationDesign,
    Proof,
}

impl std::fmt::Display for GoalPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalPhase::Intake => "intake",
            GoalPhase::Planning => "planning",
            GoalPhase::Decomposition => "decomposition",
            GoalPhase::Execution => "execution",
            GoalPhase::VerificationDesign => "verification_design",
            GoalPhase::Proof => "proof",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTerminalCriteria {
    pub proof_required: bool,
    pub gates_required: bool,
    pub human_blockers_stop: bool,
}

impl Default for GoalTerminalCriteria {
    fn default() -> Self {
        Self {
            proof_required: true,
            gates_required: true,
            human_blockers_stop: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalFailure {
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalArtifact {
    pub kind: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalTaskStatus {
    Pending,
    Blocked,
    Done,
}

impl std::fmt::Display for GoalTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalTaskStatus::Pending => "pending",
            GoalTaskStatus::Blocked => "blocked",
            GoalTaskStatus::Done => "done",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskEvidence {
    pub kind: String,
    pub path: PathBuf,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: GoalTaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub evidence: Vec<GoalTaskEvidence>,
    pub dependencies: Vec<String>,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub risk: String,
    pub acceptance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskGraph {
    pub version: u32,
    pub goal_id: String,
    pub generated_at: DateTime<Utc>,
    pub tasks: Vec<GoalTask>,
}

impl GoalTaskGraph {
    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_TASK_GRAPH_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal task graph: {}", path.display()))?;
        let graph = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal task graph: {}", path.display()))?;
        Ok(graph)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTaskGraphSummary {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub blocked_tasks: usize,
    pub done_tasks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalGitEvidence {
    pub branch: String,
    pub head: String,
    pub dirty: bool,
}

#[derive(Debug, Clone)]
struct GoalAgentRunEvidence {
    summary: RunSummary,
    run_path: PathBuf,
    task_policy_path: PathBuf,
    agent_task_proposals_path: PathBuf,
    worker_outbox_path: PathBuf,
    wire_events_path: PathBuf,
    mutation_diff_path: PathBuf,
    changed_files_path: PathBuf,
    changed_files: Vec<String>,
    accepted_task_count: usize,
    rejected_task_count: usize,
    accepted_task_ids: Vec<String>,
    agent_proposed_tasks: Vec<GoalAgentTaskProposal>,
    worker_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoalAgentTaskProposal {
    id: String,
    title: String,
    description: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    read_set: Vec<String>,
    #[serde(default)]
    write_set: Vec<String>,
    #[serde(default = "default_goal_agent_task_risk")]
    risk: String,
    #[serde(default)]
    acceptance: Vec<String>,
    #[serde(default = "default_goal_agent_task_budget_secs")]
    budget_secs: u64,
    #[serde(default)]
    priority: i32,
}

#[derive(Debug, Clone, Serialize)]
struct GoalAgentRejectedTask {
    task: GoalAgentTaskProposal,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
struct GoalAgentTaskPolicy {
    goal_id: String,
    run_id: String,
    max_agents: usize,
    proposed_tasks: Vec<GoalAgentTaskProposal>,
    accepted_tasks: Vec<GoalAgentTaskProposal>,
    rejected_tasks: Vec<GoalAgentRejectedTask>,
}

fn default_goal_agent_task_risk() -> String {
    "moderate".to_string()
}

fn default_goal_agent_task_budget_secs() -> u64 {
    300
}

#[derive(Debug, Clone)]
struct GoalReviewEvidence {
    review_path: PathBuf,
    security_review_path: PathBuf,
    review_summary: String,
    security_summary: String,
    security_findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProof {
    pub version: u32,
    pub goal_id: String,
    pub status: GoalStatus,
    pub readiness: String,
    pub summary: String,
    pub generated_at: DateTime<Utc>,
    pub artifacts: Vec<GoalArtifact>,
    pub task_graph_summary: GoalTaskGraphSummary,
    pub changed_files: Vec<String>,
    pub commits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GoalGitEvidence>,
    pub gates: Vec<GateResult>,
    #[serde(default)]
    pub post_mutation_gates_ran: bool,
    pub known_gaps: Vec<String>,
    pub human_decisions_required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalState {
    #[serde(default = "default_goal_version")]
    pub version: u32,
    pub goal_id: String,
    pub original_goal: String,
    pub normalized_goal: String,
    pub status: GoalStatus,
    #[serde(default = "default_goal_phase")]
    pub phase: GoalPhase,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    pub until_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_agents: Option<usize>,
    pub terminal_criteria: GoalTerminalCriteria,
    #[serde(default)]
    pub artifacts: Vec<GoalArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<GoalFailure>,
    pub state_dir: PathBuf,
}

fn default_goal_version() -> u32 {
    1
}

fn default_goal_phase() -> GoalPhase {
    GoalPhase::Intake
}

#[derive(Debug, Clone)]
pub struct CreateGoalOptions {
    pub until_ready: bool,
    pub budget_time: Option<String>,
    pub max_agents: Option<usize>,
}

impl GoalState {
    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join(GOAL_STATE_FILE)
    }

    pub async fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&self.state_file(), json.as_bytes()).await
    }

    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_STATE_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal state: {}", path.display()))?;
        let state = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal state: {}", path.display()))?;
        Ok(state)
    }
}

impl GoalProof {
    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_PROOF_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal proof: {}", path.display()))?;
        let proof = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal proof: {}", path.display()))?;
        Ok(proof)
    }
}

pub fn goals_dir() -> PathBuf {
    crate::runtime::config::omk_state_dir().join(GOALS_DIR)
}

pub async fn create_goal(goal: &str, options: CreateGoalOptions) -> Result<GoalState> {
    create_goal_with_scaffold(goal, options).await
}

pub async fn plan_goal(goal: &str) -> Result<GoalState> {
    create_goal_with_scaffold(
        goal,
        CreateGoalOptions {
            until_ready: false,
            budget_time: None,
            max_agents: None,
        },
    )
    .await
}

async fn create_goal_with_scaffold(goal: &str, options: CreateGoalOptions) -> Result<GoalState> {
    let id = generate_goal_id();
    let goal_dir = goals_dir().join(&id);
    crate::runtime::config::ensure_private_dir(&goal_dir).await?;

    let now = Utc::now();
    let state = GoalState {
        version: 1,
        goal_id: id.clone(),
        original_goal: goal.to_string(),
        normalized_goal: normalize_goal(goal),
        status: GoalStatus::NotReady,
        phase: GoalPhase::Intake,
        created_at: now,
        updated_at: now,
        completed_at: Some(now),
        until_ready: options.until_ready,
        budget_time: options.budget_time,
        max_agents: options.max_agents,
        terminal_criteria: GoalTerminalCriteria::default(),
        artifacts: Vec::new(),
        failure: None,
        state_dir: goal_dir.clone(),
    };
    state.save().await?;

    run_controller_scaffold(state).await
}

async fn run_controller_scaffold(mut state: GoalState) -> Result<GoalState> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    writer
        .append(&builder.run_started("goal", &cwd, &state.original_goal)?)
        .await?;

    let now = Utc::now();
    state.phase = GoalPhase::Planning;
    write_goal_brief(&state, now).await?;
    record_artifact(&mut state, "prd", GOAL_PRD_FILE, now);

    state.phase = GoalPhase::Planning;
    write_technical_plan(&state, now).await?;
    record_artifact(&mut state, "technical_plan", GOAL_TECHNICAL_PLAN_FILE, now);

    state.phase = GoalPhase::Decomposition;
    let task_graph = write_task_graph(&state, now).await?;
    record_artifact(&mut state, "task_graph", GOAL_TASK_GRAPH_FILE, now);

    state.phase = GoalPhase::VerificationDesign;
    write_test_spec(&state, &task_graph, now).await?;
    record_artifact(&mut state, "test_spec", GOAL_TEST_SPEC_FILE, now);
    append_controller_task_events(&state, &task_graph).await?;

    state.phase = GoalPhase::Proof;
    let git = detect_git_evidence(&cwd).await;
    let proof = build_scaffold_proof(&state, &task_graph, git, now);
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;
    record_artifact(&mut state, "proof", GOAL_PROOF_FILE, now);

    state.status = GoalStatus::NotReady;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;

    writer
        .append(
            &builder.run_failed(
                "goal controller scaffold created; run omk goal execute to launch the bounded agent wave",
            )?,
        )
        .await?;

    Ok(state)
}

pub async fn list_goals() -> Result<Vec<GoalState>> {
    let dir = goals_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut goals = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            match GoalState::load(&entry.path()).await {
                Ok(state) => goals.push(state),
                Err(error) => tracing::warn!(
                    path = %entry.path().display(),
                    error = %error,
                    "Skipping unreadable goal state"
                ),
            }
        }
    }

    goals.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.goal_id.cmp(&a.goal_id))
    });
    Ok(goals)
}

pub async fn resolve_goal(goal_id: &str) -> Result<GoalState> {
    if goal_id == "latest" {
        let mut goals = list_goals().await?;
        if let Some(goal) = goals.drain(..).next() {
            return Ok(goal);
        }
        anyhow::bail!("No goals found");
    }

    let goal_dir = goals_dir().join(goal_id);
    if !goal_dir.exists() {
        anyhow::bail!("Goal '{}' not found", goal_id);
    }
    GoalState::load(&goal_dir).await
}

pub async fn resolve_goal_proof(goal_id: &str) -> Result<GoalProof> {
    let goal = resolve_goal(goal_id).await?;
    GoalProof::load(&goal.state_dir).await
}

pub async fn verify_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = resolve_goal(goal_id).await?;
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let gate_config = load_or_detect_gates(project_dir).await;
    let gate_artifacts = state
        .state_dir
        .join(GOAL_ARTIFACTS_DIR)
        .join(GOAL_GATE_ARTIFACTS_DIR);
    let gates = run_gates_with_evidence(&gate_config, project_dir, Some(&gate_artifacts)).await;
    let changed_files = detect_changed_files(project_dir).await;
    let now = Utc::now();

    append_gate_events(&state, &gates).await?;
    let git = detect_git_evidence(project_dir).await;
    let updated_task = apply_local_verification_task_result(&mut task_graph, &gates, now);
    if let Some(task) = &updated_task {
        append_local_verification_task_events(&state, task).await?;
    }
    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &task_graph).await?;

    state.status = GoalStatus::NotReady;
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;

    let proof = build_verified_proof(&state, &task_graph, gates, changed_files, git, false, now);
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;

    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    writer
        .append(&builder.proof_written(
            &state.state_dir.join(GOAL_PROOF_FILE),
            &proof.status.to_string(),
        )?)
        .await?;

    Ok(proof)
}

pub async fn execute_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = resolve_goal(goal_id).await?;
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::Execution;
    state.updated_at = Utc::now();
    state.completed_at = None;
    state.save().await?;

    let verification_proof = verify_goal(goal_id, project_dir).await?;
    let mut state = resolve_goal(goal_id).await?;
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let local_verify_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID && task.status == GoalTaskStatus::Done);

    if !local_verify_done {
        return Ok(verification_proof);
    }

    let now = Utc::now();
    let agent_evidence =
        run_goal_agent_execution_task(&state, &task_graph, project_dir, now).await?;
    if let Some(task) = apply_agent_execution_task_result(&mut task_graph, &agent_evidence, now) {
        append_agent_execution_task_events(&state, &task, &agent_evidence).await?;
    }
    apply_agent_proposed_task_mutations(&state, &mut task_graph, &agent_evidence, now).await?;

    let agent_execution_succeeded = agent_evidence.summary.completed
        == agent_evidence.summary.total
        && agent_evidence.summary.failed == 0;
    let mut proof_gates = verification_proof.gates;
    let mut proof_git = verification_proof.git;
    let mut proof_changed_files = agent_evidence.changed_files;
    let mut post_mutation_gates_ran = false;
    if agent_execution_succeeded && !proof_changed_files.is_empty() {
        let gate_config = load_or_detect_gates(project_dir).await;
        let gate_artifacts = state
            .state_dir
            .join(GOAL_ARTIFACTS_DIR)
            .join(GOAL_GATE_ARTIFACTS_DIR)
            .join("post-mutation");
        proof_gates =
            run_gates_with_evidence(&gate_config, project_dir, Some(&gate_artifacts)).await;
        append_gate_events(&state, &proof_gates).await?;
        proof_git = detect_git_evidence(project_dir).await;
        proof_changed_files = detect_changed_files(project_dir).await;
        if let Some(task) = apply_local_verification_task_result(&mut task_graph, &proof_gates, now)
        {
            append_local_verification_task_events(&state, &task).await?;
        }
        post_mutation_gates_ran = true;
    }

    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &task_graph).await?;

    record_artifact_path_once(
        &mut state,
        "agent_run",
        agent_evidence.run_path.clone(),
        now,
    );
    state.status = GoalStatus::NotReady;
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;

    let proof = build_verified_proof(
        &state,
        &task_graph,
        proof_gates,
        proof_changed_files,
        proof_git,
        post_mutation_gates_ran,
        now,
    );
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;

    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    writer
        .append(&builder.proof_written(
            &state.state_dir.join(GOAL_PROOF_FILE),
            &proof.status.to_string(),
        )?)
        .await?;

    Ok(proof)
}

pub async fn review_goal(goal_id: &str, project_dir: &Path) -> Result<GoalProof> {
    let mut state = resolve_goal(goal_id).await?;
    state.status = GoalStatus::Running;
    state.phase = GoalPhase::VerificationDesign;
    state.updated_at = Utc::now();
    state.completed_at = None;
    state.save().await?;

    let mut state = resolve_goal(goal_id).await?;
    let mut task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let prior_proof = GoalProof::load(&state.state_dir).await?;
    let now = Utc::now();

    let review_evidence =
        write_goal_review_evidence(&state, &task_graph, &prior_proof, project_dir, now).await?;
    let mut updated_tasks = Vec::new();
    if let Some(task) = apply_goal_review_task_result(&mut task_graph, &review_evidence, now) {
        updated_tasks.push(task);
    }
    if let Some(task) =
        apply_goal_security_review_task_result(&mut task_graph, &review_evidence, now)
    {
        updated_tasks.push(task);
    }
    append_goal_review_task_events(&state, &updated_tasks).await?;
    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &task_graph).await?;

    record_artifact_path_once(
        &mut state,
        "review",
        review_evidence.review_path.clone(),
        now,
    );
    record_artifact_path_once(
        &mut state,
        "security_review",
        review_evidence.security_review_path.clone(),
        now,
    );
    state.status = GoalStatus::NotReady;
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;

    let proof = build_verified_proof(
        &state,
        &task_graph,
        prior_proof.gates,
        prior_proof.changed_files,
        prior_proof.git,
        prior_proof.post_mutation_gates_ran,
        now,
    );
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;

    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    writer
        .append(&builder.proof_written(
            &state.state_dir.join(GOAL_PROOF_FILE),
            &proof.status.to_string(),
        )?)
        .await?;

    Ok(proof)
}

async fn run_goal_agent_execution_task(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    project_dir: &Path,
    started_at: DateTime<Utc>,
) -> Result<GoalAgentRunEvidence> {
    let run_id = format!("{}-{}", state.goal_id, GOAL_AGENT_EXECUTE_TASK_ID);
    let run_path = PathBuf::from(GOAL_ARTIFACTS_DIR)
        .join(GOAL_AGENT_RUNS_DIR)
        .join(GOAL_AGENT_EXECUTE_TASK_ID);
    let run_dir = state.state_dir.join(&run_path);
    crate::runtime::config::ensure_private_dir(&run_dir).await?;

    let worker_dir = run_dir.join(WORKERS_DIR).join(GOAL_AGENT_WORKER_ID);
    crate::runtime::config::ensure_private_dir(&worker_dir).await?;

    let worker_outbox_path = run_path
        .join(WORKERS_DIR)
        .join(GOAL_AGENT_WORKER_ID)
        .join(OUTBOX_FILE);
    let wire_events_path = run_path
        .join(WORKERS_DIR)
        .join(GOAL_AGENT_WORKER_ID)
        .join("wire-events.jsonl");
    let task_policy_path = run_path.join(GOAL_AGENT_TASK_POLICY_FILE);
    let agent_task_proposals_path = run_path.join(GOAL_AGENT_TASK_PROPOSALS_FILE);
    let mutation_diff_path = run_path.join("mutation.diff");
    let changed_files_path = run_path.join("changed-files.json");

    let spec = WorkerSpec {
        name: GOAL_AGENT_WORKER_ID.to_string(),
        role: GOAL_AGENT_WORKER_ROLE.to_string(),
        inbox: worker_dir.join(INBOX_FILE),
        outbox: worker_dir.join(OUTBOX_FILE),
        heartbeat: worker_dir.join(HEARTBEAT_FILE),
        project_dir: Some(project_dir.to_path_buf()),
    };
    spec.save().await?;
    tokio::fs::write(&spec.inbox, b"").await?;
    tokio::fs::write(&spec.outbox, b"").await?;
    tokio::fs::write(worker_dir.join("wire-events.jsonl"), b"").await?;

    let event_writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(run_id.clone()));
    let proposals = propose_goal_agent_tasks(state);
    let policy = validate_goal_agent_task_proposals(state, task_graph, &run_id, proposals);
    write_json_artifact(&state.state_dir.join(&task_policy_path), &policy).await?;
    append_goal_agent_task_policy_events(&event_writer, &run_id, &policy).await?;

    let accepted_task_ids: Vec<String> = policy
        .accepted_tasks
        .iter()
        .map(|task| task.id.clone())
        .collect();
    let accepted_task_count = policy.accepted_tasks.len();
    let rejected_task_count = policy.rejected_tasks.len();
    let scheduler_tasks =
        goal_agent_scheduler_tasks(state, task_graph, started_at, &policy.accepted_tasks);
    let run_description = format!(
        "goal controller agent wave: accepted={}, rejected={}, max_agents={}",
        accepted_task_count, rejected_task_count, policy.max_agents
    );

    event_writer
        .append(&builder.run_started("goal-agent", project_dir, &run_description)?)
        .await?;

    if scheduler_tasks.is_empty() {
        let summary =
            "Goal controller rejected all proposed agent tasks; no safe work is dispatchable";
        event_writer.append(&builder.run_failed(summary)?).await?;
        let changed_files = write_goal_agent_mutation_snapshot(
            state,
            project_dir,
            &mutation_diff_path,
            &changed_files_path,
        )
        .await?;
        return Ok(GoalAgentRunEvidence {
            summary: RunSummary {
                run_id,
                completed: 0,
                failed: 1,
                cancelled: 0,
                total: 1,
            },
            run_path,
            task_policy_path,
            agent_task_proposals_path,
            worker_outbox_path,
            wire_events_path,
            mutation_diff_path,
            changed_files_path,
            changed_files,
            accepted_task_count,
            rejected_task_count,
            accepted_task_ids,
            agent_proposed_tasks: Vec::new(),
            worker_summary: Some(summary.to_string()),
        });
    }

    if !goal_agent_wire_runtime_available() {
        let summary = "Kimi CLI not found; install/authenticate kimi or set MOCK_KIMI to a mock binary before running goal agent execution";
        event_writer.append(&builder.run_failed(summary)?).await?;
        let changed_files = write_goal_agent_mutation_snapshot(
            state,
            project_dir,
            &mutation_diff_path,
            &changed_files_path,
        )
        .await?;
        return Ok(GoalAgentRunEvidence {
            summary: RunSummary {
                run_id,
                completed: 0,
                failed: accepted_task_count,
                cancelled: 0,
                total: accepted_task_count,
            },
            run_path,
            task_policy_path,
            agent_task_proposals_path,
            worker_outbox_path,
            wire_events_path,
            mutation_diff_path,
            changed_files_path,
            changed_files,
            accepted_task_count,
            rejected_task_count,
            accepted_task_ids,
            agent_proposed_tasks: Vec::new(),
            worker_summary: Some(summary.to_string()),
        });
    }

    event_writer
        .append(&builder.worker_started(
            WorkerId(GOAL_AGENT_WORKER_ID.to_string()),
            GOAL_AGENT_WORKER_ROLE,
        )?)
        .await?;

    let cancel = CancellationToken::new();
    let adapter = WireWorkerAdapter::new_with_cancel(
        spec.clone(),
        RunId(run_id.clone()),
        event_writer.clone(),
        cancel.clone(),
    );
    let mut handle = adapter.spawn();
    let mut runner = TeamRunner::init_with_tasks(
        &run_id,
        project_dir,
        &run_dir,
        event_writer,
        scheduler_tasks,
    )
    .await?;

    let run_result = runner.run(std::slice::from_ref(&spec)).await;
    cancel.cancel();
    stop_wire_worker(&mut handle).await;

    let summary = run_result?;
    let worker_results = read_goal_agent_worker_results(&spec, &accepted_task_ids).await?;
    let worker_summary = summarize_goal_agent_worker_results(&worker_results);
    let agent_proposed_tasks = extract_goal_agent_task_proposals(&worker_results);
    let changed_files = write_goal_agent_mutation_snapshot(
        state,
        project_dir,
        &mutation_diff_path,
        &changed_files_path,
    )
    .await?;

    Ok(GoalAgentRunEvidence {
        summary,
        run_path,
        task_policy_path,
        agent_task_proposals_path,
        worker_outbox_path,
        wire_events_path,
        mutation_diff_path,
        changed_files_path,
        changed_files,
        accepted_task_count,
        rejected_task_count,
        accepted_task_ids,
        agent_proposed_tasks,
        worker_summary,
    })
}

fn propose_goal_agent_tasks(state: &GoalState) -> Vec<GoalAgentTaskProposal> {
    vec![
        GoalAgentTaskProposal {
            id: GOAL_AGENT_IMPLEMENT_TASK_ID.to_string(),
            title: "Implement bounded goal slice".to_string(),
            description: "Make one bounded in-repository project change that moves the goal forward, then summarize changed files and remaining verification needs.".to_string(),
            dependencies: Vec::new(),
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
            ],
            write_set: vec![
                "project files".to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                GOAL_PROOF_FILE.to_string(),
            ],
            risk: "moderate".to_string(),
            acceptance: vec![
                "Make only a bounded project change needed to move the goal forward.".to_string(),
                "Do not commit, publish, or touch secrets.".to_string(),
                "Summarize changed files and verification still needed for production readiness.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 900),
            priority: 20,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_VERIFY_TASK_ID.to_string(),
            title: "Verify agent wave follow-up".to_string(),
            description: "Inspect the bounded implementation result and summarize verification, review, or hardening follow-up that still blocks readiness.".to_string(),
            dependencies: vec![GOAL_AGENT_IMPLEMENT_TASK_ID.to_string()],
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                "project files".to_string(),
            ],
            write_set: vec![GOAL_ARTIFACTS_DIR.to_string()],
            risk: "low".to_string(),
            acceptance: vec![
                "Review the bounded project change and call out remaining verification gaps.".to_string(),
                "Do not make broad follow-up mutations without a new controller-approved task.".to_string(),
                "Keep the goal proof honest when production readiness is still blocked.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 10,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_PUBLISH_TASK_ID.to_string(),
            title: "Publish crate release".to_string(),
            description: "Publish this crate to crates.io after the agent wave succeeds.".to_string(),
            dependencies: vec![GOAL_AGENT_VERIFY_TASK_ID.to_string()],
            read_set: vec![GOAL_PROOF_FILE.to_string()],
            write_set: vec!["crates.io".to_string()],
            risk: "external-side-effect".to_string(),
            acceptance: vec!["Publish the package to crates.io.".to_string()],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 0,
        },
    ]
}

fn goal_agent_task_budget_secs(state: &GoalState, requested_secs: u64) -> u64 {
    let Some(total_budget_secs) = state.budget_time.as_deref().and_then(parse_duration_secs) else {
        return requested_secs;
    };
    let per_task_ceiling = if total_budget_secs < 60 {
        total_budget_secs.max(1)
    } else {
        (total_budget_secs / 4).max(60)
    };
    requested_secs.min(per_task_ceiling)
}

fn parse_duration_secs(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (number, multiplier) = match trimmed.chars().last()? {
        's' | 'S' => (&trimmed[..trimmed.len() - 1], 1),
        'm' | 'M' => (&trimmed[..trimmed.len() - 1], 60),
        'h' | 'H' => (&trimmed[..trimmed.len() - 1], 60 * 60),
        'd' | 'D' => (&trimmed[..trimmed.len() - 1], 24 * 60 * 60),
        _ => (trimmed, 1),
    };
    number.trim().parse::<u64>().ok()?.checked_mul(multiplier)
}

fn validate_goal_agent_task_proposals(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    run_id: &str,
    proposals: Vec<GoalAgentTaskProposal>,
) -> GoalAgentTaskPolicy {
    let existing_task_ids: HashSet<String> = task_graph
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect();
    let mut known_dependencies: HashSet<String> = task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Done)
        .map(|task| task.id.clone())
        .collect();
    let mut seen_ids = HashSet::new();
    let mut accepted_tasks = Vec::new();
    let mut rejected_tasks = Vec::new();

    for proposal in proposals.iter().cloned() {
        if let Some(reason) = reject_goal_agent_task_proposal(
            &proposal,
            &mut seen_ids,
            &known_dependencies,
            &existing_task_ids,
        ) {
            rejected_tasks.push(GoalAgentRejectedTask {
                task: proposal,
                reason,
            });
            continue;
        }

        known_dependencies.insert(proposal.id.clone());
        accepted_tasks.push(proposal);
    }

    GoalAgentTaskPolicy {
        goal_id: state.goal_id.clone(),
        run_id: run_id.to_string(),
        max_agents: state.max_agents.unwrap_or(1).max(1),
        proposed_tasks: proposals,
        accepted_tasks,
        rejected_tasks,
    }
}

fn reject_goal_agent_task_proposal(
    proposal: &GoalAgentTaskProposal,
    seen_ids: &mut HashSet<String>,
    known_dependencies: &HashSet<String>,
    existing_task_ids: &HashSet<String>,
) -> Option<String> {
    if proposal.id.trim().is_empty() {
        return Some("task id must not be empty".to_string());
    }
    if proposal.title.trim().is_empty() {
        return Some("task title must not be empty".to_string());
    }
    if proposal.description.trim().is_empty() {
        return Some("task description must not be empty".to_string());
    }
    if !seen_ids.insert(proposal.id.clone()) {
        return Some("duplicate task id rejected by goal policy".to_string());
    }
    if existing_task_ids.contains(&proposal.id) {
        return Some("task id already exists in the goal graph".to_string());
    }
    if proposal.budget_secs == 0 {
        return Some("task budget must be greater than zero".to_string());
    }

    let policy_text = format!(
        "{} {} {}",
        proposal.id,
        proposal.description,
        proposal.write_set.join(" ")
    )
    .to_ascii_lowercase();
    if policy_text.contains("crates.io") || policy_text.contains("publish") {
        return Some("publishing is disabled for GitHub-only goal execution".to_string());
    }

    if let Some(path) = proposal
        .read_set
        .iter()
        .chain(proposal.write_set.iter())
        .find(|path| !is_safe_goal_agent_path(path))
    {
        return Some(format!(
            "path is outside the allowed goal policy roots: {path}"
        ));
    }

    if let Some(dep) = proposal
        .dependencies
        .iter()
        .find(|dep| !known_dependencies.contains(*dep))
    {
        return Some(format!("dependency is not accepted or completed: {dep}"));
    }

    None
}

fn is_safe_goal_agent_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "project files" {
        return true;
    }
    let path = Path::new(trimmed);
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && trimmed != ".git"
        && !trimmed.starts_with(".git/")
}

async fn append_goal_agent_task_policy_events(
    writer: &EventWriter,
    run_id: &str,
    policy: &GoalAgentTaskPolicy,
) -> Result<()> {
    for proposal in &policy.proposed_tasks {
        let event = Event::new(RunId(run_id.to_string()), EventKind::TaskProposed)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(proposal, None))?;
        writer.append(&event).await?;
    }

    for proposal in &policy.accepted_tasks {
        let event = Event::new(RunId(run_id.to_string()), EventKind::TaskAccepted)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(
                proposal,
                Some("accepted by goal policy"),
            ))?;
        writer.append(&event).await?;
    }

    for decision in &policy.rejected_tasks {
        let event = Event::new(RunId(run_id.to_string()), EventKind::TaskRejected)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(
                &decision.task,
                Some(&decision.reason),
            ))?;
        writer.append(&event).await?;
    }

    Ok(())
}

fn goal_agent_task_policy_payload(
    proposal: &GoalAgentTaskProposal,
    reason: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "task_id": proposal.id,
        "title": proposal.title,
        "risk": proposal.risk,
        "budget_secs": proposal.budget_secs,
        "dependencies": proposal.dependencies,
        "read_set": proposal.read_set,
        "write_set": proposal.write_set,
        "reason": reason,
    })
}

fn goal_agent_scheduler_tasks(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
    proposals: &[GoalAgentTaskProposal],
) -> Vec<Task> {
    proposals
        .iter()
        .map(|proposal| {
            let mut task = Task::new(proposal.id.clone(), proposal.title.clone())
                .with_description(goal_agent_task_prompt(
                    state,
                    task_graph,
                    generated_at,
                    proposal,
                ))
                .with_dependencies(proposal.dependencies.clone())
                .with_read_set(proposal.read_set.clone())
                .with_write_set(proposal.write_set.clone())
                .with_priority(proposal.priority)
                .with_max_retries(0);
            task.extra.insert(
                "acceptance".to_string(),
                serde_json::json!(proposal.acceptance),
            );
            task.extra.insert(
                "budget_secs".to_string(),
                serde_json::json!(proposal.budget_secs),
            );
            task.extra
                .insert("risk".to_string(), serde_json::json!(proposal.risk));
            task.extra.insert(
                "controller_task_id".to_string(),
                serde_json::json!(GOAL_AGENT_EXECUTE_TASK_ID),
            );
            task
        })
        .collect()
}

fn goal_agent_task_prompt(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
    proposal: &GoalAgentTaskProposal,
) -> String {
    let local_status = task_graph
        .tasks
        .iter()
        .find(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID)
        .map(|task| task.status.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "Goal ID: {}\nGenerated: {generated_at}\n\nOriginal goal:\n{}\n\nNormalized goal:\n{}\n\nController task: {}\nTitle: {}\nBudget: {} seconds\nRisk: {}\n\nTask:\n{}\n\nAcceptance criteria:\n- {}\n\nPolicy:\nStay inside the current repository, keep the diff minimal, do not commit, do not publish, do not touch secrets, and summarize changed files plus verification still needed for production readiness.\n\nLocal verification task status: {local_status}",
        state.goal_id,
        state.original_goal,
        state.normalized_goal,
        proposal.id,
        proposal.title,
        proposal.budget_secs,
        proposal.risk,
        proposal.description,
        proposal.acceptance.join("\n- ")
    )
}

async fn write_goal_agent_mutation_snapshot(
    state: &GoalState,
    project_dir: &Path,
    mutation_diff_path: &Path,
    changed_files_path: &Path,
) -> Result<Vec<String>> {
    let changed_files = detect_changed_files(project_dir).await;
    let diff = git_diff(project_dir).await.unwrap_or_default();
    let body = if diff.trim().is_empty() {
        if changed_files.is_empty() {
            "No project file changes were detected after the agent wave.\n".to_string()
        } else {
            format!(
                "No tracked git diff was available. Changed files:\n{}\n",
                changed_files
                    .iter()
                    .map(|file| format!("- {file}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    } else {
        diff
    };

    write_text_artifact(&state.state_dir.join(mutation_diff_path), &body).await?;
    write_json_artifact(&state.state_dir.join(changed_files_path), &changed_files).await?;
    Ok(changed_files)
}

async fn git_diff(project_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["diff", "--no-ext-diff", "--"])
        .current_dir(project_dir)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn goal_agent_wire_runtime_available() -> bool {
    std::env::var_os("MOCK_KIMI").is_some() || which::which("kimi").is_ok()
}

async fn stop_wire_worker(handle: &mut JoinHandle<()>) {
    if tokio::time::timeout(Duration::from_secs(2), &mut *handle)
        .await
        .is_err()
    {
        handle.abort();
        let _ = handle.await;
    }
}

async fn read_goal_agent_worker_results(
    spec: &WorkerSpec,
    task_ids: &[String],
) -> Result<Vec<WorkerResult>> {
    let results: Vec<WorkerResult> = spec.read_results().await?;
    Ok(results
        .into_iter()
        .filter(|result| task_ids.iter().any(|task_id| task_id == &result.task_id))
        .collect())
}

fn summarize_goal_agent_worker_results(results: &[WorkerResult]) -> Option<String> {
    let summaries: Vec<String> = results
        .iter()
        .map(|result| format!("{}: {}", result.task_id, result.summary))
        .collect();
    (!summaries.is_empty()).then(|| summaries.join(" | "))
}

fn extract_goal_agent_task_proposals(results: &[WorkerResult]) -> Vec<GoalAgentTaskProposal> {
    results
        .iter()
        .flat_map(|result| extract_goal_agent_task_proposals_from_text(&result.summary))
        .collect()
}

fn extract_goal_agent_task_proposals_from_text(summary: &str) -> Vec<GoalAgentTaskProposal> {
    let mut proposals = Vec::new();
    let mut search = summary;
    while let Some(marker_pos) = search.find(GOAL_AGENT_TASK_PROPOSAL_MARKER) {
        let after_marker = &search[marker_pos + GOAL_AGENT_TASK_PROPOSAL_MARKER.len()..];
        let Some((json, consumed)) = extract_first_json_object(after_marker) else {
            break;
        };
        match serde_json::from_str::<GoalAgentTaskProposal>(&json) {
            Ok(proposal) => proposals.push(proposal),
            Err(error) => tracing::warn!(
                error = %error,
                "Ignoring malformed agent task proposal"
            ),
        }
        search = &after_marker[consumed..];
    }
    proposals
}

fn extract_first_json_object(input: &str) -> Option<(String, usize)> {
    let start = input.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in input[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some((input[start..end].to_string(), end));
                }
            }
            _ => {}
        }
    }

    None
}

fn apply_agent_execution_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalAgentRunEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_AGENT_EXECUTE_TASK_ID)?;
    let success =
        evidence.summary.completed == evidence.summary.total && evidence.summary.failed == 0;

    task.status = if success {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_AGENT_WORKER_ROLE.to_string());
    task.completed_at = success.then_some(completed_at);
    task.evidence = agent_execution_task_evidence(evidence, success);
    Some(task.clone())
}

async fn apply_agent_proposed_task_mutations(
    state: &GoalState,
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalAgentRunEvidence,
    recorded_at: DateTime<Utc>,
) -> Result<()> {
    if evidence.agent_proposed_tasks.is_empty() {
        return Ok(());
    }

    let policy = validate_goal_agent_task_proposals(
        state,
        task_graph,
        &evidence.summary.run_id,
        evidence.agent_proposed_tasks.clone(),
    );
    write_json_artifact(
        &state.state_dir.join(&evidence.agent_task_proposals_path),
        &policy,
    )
    .await?;
    append_agent_proposed_task_events(state, evidence, &policy).await?;

    for proposal in &policy.accepted_tasks {
        task_graph.tasks.push(goal_task_from_agent_proposal(
            proposal,
            &evidence.agent_task_proposals_path,
            recorded_at,
        ));
    }

    Ok(())
}

fn goal_task_from_agent_proposal(
    proposal: &GoalAgentTaskProposal,
    proposal_path: &Path,
    recorded_at: DateTime<Utc>,
) -> GoalTask {
    GoalTask {
        id: proposal.id.clone(),
        title: proposal.title.clone(),
        description: proposal.description.clone(),
        status: GoalTaskStatus::Pending,
        owner_role: Some(GOAL_AGENT_WORKER_ROLE.to_string()),
        completed_at: None,
        evidence: vec![GoalTaskEvidence {
            kind: "agent_proposal".to_string(),
            path: proposal_path.to_path_buf(),
            summary: format!(
                "Accepted agent-proposed follow-up task at {recorded_at}: {}",
                proposal.title
            ),
        }],
        dependencies: proposal.dependencies.clone(),
        read_set: proposal.read_set.clone(),
        write_set: proposal.write_set.clone(),
        risk: proposal.risk.clone(),
        acceptance: proposal.acceptance.clone(),
    }
}

async fn append_agent_proposed_task_events(
    state: &GoalState,
    evidence: &GoalAgentRunEvidence,
    policy: &GoalAgentTaskPolicy,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let run_id = &evidence.summary.run_id;

    for proposal in &policy.proposed_tasks {
        let event = Event::new(RunId(run_id.to_string()), EventKind::TaskProposed)
            .with_actor(GOAL_AGENT_WORKER_ID)
            .with_payload(goal_agent_task_policy_payload(proposal, None))?;
        writer.append(&event).await?;
    }

    for proposal in &policy.accepted_tasks {
        let event = Event::new(RunId(run_id.to_string()), EventKind::TaskAccepted)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(
                proposal,
                Some("accepted agent-proposed task graph mutation"),
            ))?;
        writer.append(&event).await?;
    }

    for decision in &policy.rejected_tasks {
        let event = Event::new(RunId(run_id.to_string()), EventKind::TaskRejected)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(
                &decision.task,
                Some(&decision.reason),
            ))?;
        writer.append(&event).await?;
    }

    Ok(())
}

fn agent_execution_task_evidence(
    evidence: &GoalAgentRunEvidence,
    success: bool,
) -> Vec<GoalTaskEvidence> {
    let status = if success { "completed" } else { "blocked" };
    let worker_summary = evidence
        .worker_summary
        .as_deref()
        .filter(|summary| !summary.trim().is_empty())
        .unwrap_or("no worker summary recorded");
    let run_summary = format!(
        "Agent execution {status}: {}/{} scheduler task(s) completed; failed={}, cancelled={}. Worker summary: {worker_summary}",
        evidence.summary.completed,
        evidence.summary.total,
        evidence.summary.failed,
        evidence.summary.cancelled
    );
    let mutation_summary = if evidence.changed_files.is_empty() {
        "No project file changes were detected after the agent wave.".to_string()
    } else {
        format!(
            "Project mutation evidence captured for changed file(s): {}",
            evidence.changed_files.join(", ")
        )
    };

    let mut task_evidence = vec![
        GoalTaskEvidence {
            kind: "agent_run".to_string(),
            path: evidence.run_path.clone(),
            summary: run_summary,
        },
        GoalTaskEvidence {
            kind: "task_policy".to_string(),
            path: evidence.task_policy_path.clone(),
            summary: format!(
                "Controller task policy recorded: accepted={}, rejected={}, accepted_task_ids={}",
                evidence.accepted_task_count,
                evidence.rejected_task_count,
                evidence.accepted_task_ids.join(", ")
            ),
        },
    ];
    if !evidence.agent_proposed_tasks.is_empty() {
        task_evidence.push(GoalTaskEvidence {
            kind: "agent_task_proposals".to_string(),
            path: evidence.agent_task_proposals_path.clone(),
            summary: format!(
                "Agent proposed {} follow-up task(s) for controller validation.",
                evidence.agent_proposed_tasks.len()
            ),
        });
    }
    task_evidence.extend([
        GoalTaskEvidence {
            kind: "worker_outbox".to_string(),
            path: evidence.worker_outbox_path.clone(),
            summary: "Worker outbox records the scheduler-visible task result.".to_string(),
        },
        GoalTaskEvidence {
            kind: "wire_events".to_string(),
            path: evidence.wire_events_path.clone(),
            summary: "Wire event stream records the agent protocol turn.".to_string(),
        },
        GoalTaskEvidence {
            kind: "mutation_diff".to_string(),
            path: evidence.mutation_diff_path.clone(),
            summary: mutation_summary,
        },
        GoalTaskEvidence {
            kind: "changed_files".to_string(),
            path: evidence.changed_files_path.clone(),
            summary: "Changed-file snapshot captured after the agent wave.".to_string(),
        },
    ]);
    task_evidence
}

async fn append_agent_execution_task_events(
    state: &GoalState,
    task: &GoalTask,
    evidence: &GoalAgentRunEvidence,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let run_id = RunId(state.goal_id.clone());
    let task_id = TaskId(task.id.clone());
    let worker_id = WorkerId(GOAL_AGENT_WORKER_ID.to_string());
    let summary = format!(
        "{} via {} (run: {}, scheduler: {})",
        controller_task_summary(task),
        GOAL_AGENT_WORKER_ID,
        evidence.run_path.display(),
        evidence.summary.run_id
    );
    let event = if task.status == GoalTaskStatus::Done {
        EventBuilder::new(run_id).task_completed(task_id, worker_id, Some(&summary))?
    } else {
        Event::new(run_id, EventKind::TaskFailed)
            .with_actor(GOAL_AGENT_WORKER_ID)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": GOAL_AGENT_WORKER_ID,
                "summary": summary,
            }))?
    };
    writer.append(&event).await
}

async fn write_goal_review_evidence(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    proof: &GoalProof,
    project_dir: &Path,
    generated_at: DateTime<Utc>,
) -> Result<GoalReviewEvidence> {
    let review_dir = PathBuf::from(GOAL_ARTIFACTS_DIR).join(GOAL_REVIEW_ARTIFACTS_DIR);
    let review_path = review_dir.join(GOAL_REVIEW_FILE);
    let security_review_path = review_dir.join(GOAL_SECURITY_REVIEW_FILE);
    let review_abs_dir = state.state_dir.join(&review_dir);
    crate::runtime::config::ensure_private_dir(&review_abs_dir).await?;

    let local_verify_done = goal_task_done(task_graph, GOAL_LOCAL_VERIFY_TASK_ID);
    let agent_execution_done = goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID);
    let gates_ok = !proof.gates.is_empty() && gates_passed(&proof.gates);
    let security_findings = scan_goal_security_findings(project_dir, &proof.changed_files).await?;

    let review_summary = if local_verify_done && agent_execution_done && gates_ok {
        "Controller review passed: local gate evidence and agent execution evidence are present."
            .to_string()
    } else {
        "Controller review blocked: local gates, proof, or agent execution evidence is incomplete."
            .to_string()
    };
    let security_summary = if !agent_execution_done {
        "Security review blocked: agent execution evidence is incomplete.".to_string()
    } else if security_findings.is_empty() {
        "Security review passed: no high-confidence secret markers found in changed files."
            .to_string()
    } else {
        format!(
            "Security review blocked: {} high-confidence secret marker(s) found.",
            security_findings.len()
        )
    };

    let task_lines = task_graph
        .tasks
        .iter()
        .map(|task| format!("- `{}`: `{}`", task.id, task.status))
        .collect::<Vec<_>>()
        .join("\n");
    let gate_lines = if proof.gates.is_empty() {
        "- no gate evidence recorded".to_string()
    } else {
        proof
            .gates
            .iter()
            .map(|gate| {
                let status = if gate.passed { "passed" } else { "failed" };
                format!("- `{}`: `{}`", gate.name, status)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let changed_file_lines = if proof.changed_files.is_empty() {
        "- no changed files reported by git diff".to_string()
    } else {
        proof
            .changed_files
            .iter()
            .map(|file| format!("- `{file}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let review_body = format!(
        "# Goal Review\n\n\
         Generated: {generated_at}\n\n\
         Goal ID: `{}`\n\n\
         ## Result\n\n\
         {review_summary}\n\n\
         ## Task Evidence\n\n\
         {task_lines}\n\n\
         ## Gate Evidence\n\n\
         {gate_lines}\n",
        state.goal_id
    );
    write_text_artifact(&state.state_dir.join(&review_path), &review_body).await?;

    let finding_lines = if security_findings.is_empty() {
        "- no findings".to_string()
    } else {
        security_findings
            .iter()
            .map(|finding| format!("- {finding}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let security_body = format!(
        "# Goal Security Review\n\n\
         Generated: {generated_at}\n\n\
         Goal ID: `{}`\n\n\
         ## Result\n\n\
         {security_summary}\n\n\
         ## Changed Files Scanned\n\n\
         {changed_file_lines}\n\n\
         ## Findings\n\n\
         {finding_lines}\n",
        state.goal_id
    );
    write_text_artifact(&state.state_dir.join(&security_review_path), &security_body).await?;

    Ok(GoalReviewEvidence {
        review_path,
        security_review_path,
        review_summary,
        security_summary,
        security_findings,
    })
}

fn apply_goal_review_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalReviewEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let review_ok = goal_task_done(task_graph, GOAL_LOCAL_VERIFY_TASK_ID)
        && goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID);
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_REVIEW_TASK_ID)?;

    task.status = if review_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = review_ok.then_some(completed_at);
    task.evidence = vec![GoalTaskEvidence {
        kind: "review".to_string(),
        path: evidence.review_path.clone(),
        summary: evidence.review_summary.clone(),
    }];
    Some(task.clone())
}

fn apply_goal_security_review_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalReviewEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let security_ok = goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID)
        && evidence.security_findings.is_empty();
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_SECURITY_REVIEW_TASK_ID)?;

    task.status = if security_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = security_ok.then_some(completed_at);
    task.evidence = vec![GoalTaskEvidence {
        kind: "security_review".to_string(),
        path: evidence.security_review_path.clone(),
        summary: evidence.security_summary.clone(),
    }];
    Some(task.clone())
}

async fn append_goal_review_task_events(state: &GoalState, tasks: &[GoalTask]) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let mut events = Vec::new();

    for task in tasks {
        let task_id = TaskId(task.id.clone());
        let summary = controller_task_summary(task);
        events.push(
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "title": task.title,
                }))?,
        );
        let finished = if task.status == GoalTaskStatus::Done {
            builder.task_completed(task_id, worker_id.clone(), Some(&summary))?
        } else {
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskFailed)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "summary": summary,
                }))?
        };
        events.push(finished);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}

fn goal_task_done(task_graph: &GoalTaskGraph, task_id: &str) -> bool {
    task_graph
        .tasks
        .iter()
        .any(|task| task.id == task_id && task.status == GoalTaskStatus::Done)
}

async fn scan_goal_security_findings(
    project_dir: &Path,
    changed_files: &[String],
) -> Result<Vec<String>> {
    let private_key = Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")?;
    let secret_assignment =
        Regex::new(r#"(?i)\b(api[_-]?key|secret|token|password)\b\s*[:=]\s*["'][^"']{16,}["']"#)?;
    let mut findings = Vec::new();

    for changed_file in changed_files {
        let Some(path) = safe_project_file_path(project_dir, changed_file) else {
            continue;
        };
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 512 * 1024 {
            continue;
        }
        let Ok(content) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            if private_key.is_match(line) || secret_assignment.is_match(line) {
                findings.push(format!(
                    "{}:{} contains a high-confidence secret marker",
                    changed_file,
                    line_index + 1
                ));
            }
        }
    }

    Ok(findings)
}

fn safe_project_file_path(project_dir: &Path, changed_file: &str) -> Option<PathBuf> {
    let path = Path::new(changed_file);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(project_dir.join(path))
}

pub async fn cancel_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = resolve_goal(goal_id).await?;
    let now = Utc::now();
    state.status = GoalStatus::Cancelled;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.failure = Some(GoalFailure {
        reason: "cancelled by user".to_string(),
        recorded_at: now,
    });
    state.save().await?;

    let failure_json = serde_json::to_string_pretty(&state)?;
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(GOAL_FAILURE_FILE),
        failure_json.as_bytes(),
    )
    .await?;

    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let run_id = RunId(state.goal_id.clone());
    let interrupted = Event::new(run_id.clone(), EventKind::ManualInterrupt).with_actor("omk-cli");
    let failed = EventBuilder::new(run_id).run_failed("cancelled by user")?;
    writer.append_many(&[interrupted, failed]).await?;

    Ok(state)
}

fn generate_goal_id() -> String {
    let suffix = Uuid::new_v4().to_string();
    format!(
        "goal-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S-%3f"),
        &suffix[..8]
    )
}

fn normalize_goal(goal: &str) -> String {
    goal.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn record_artifact(state: &mut GoalState, kind: &str, path: &str, created_at: DateTime<Utc>) {
    state.artifacts.push(GoalArtifact {
        kind: kind.to_string(),
        path: PathBuf::from(path),
        created_at,
    });
}

fn record_artifact_path_once(
    state: &mut GoalState,
    kind: &str,
    path: PathBuf,
    created_at: DateTime<Utc>,
) {
    if state
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == kind && artifact.path == path)
    {
        return;
    }
    state.artifacts.push(GoalArtifact {
        kind: kind.to_string(),
        path,
        created_at,
    });
}

async fn write_goal_brief(state: &GoalState, generated_at: DateTime<Utc>) -> Result<()> {
    let body = format!(
        "# Goal Brief\n\n\
         Generated: {generated_at}\n\n\
         ## Original Goal\n\n\
         {}\n\n\
         ## Normalized Goal\n\n\
         {}\n\n\
         ## Current Scope\n\n\
         This scaffold captures intent and durable planning artifacts. Run `omk goal execute` to attach local gate evidence and a bounded Wire-backed agent wave, then `omk goal review` to attach controller review/security evidence.\n",
        state.original_goal, state.normalized_goal
    );
    write_text_artifact(&state.state_dir.join(GOAL_PRD_FILE), &body).await
}

async fn write_technical_plan(state: &GoalState, generated_at: DateTime<Utc>) -> Result<()> {
    let body = format!(
        "# Technical Plan\n\n\
         Generated: {generated_at}\n\n\
         ## Controller Phases\n\n\
         1. Intake records the original user goal.\n\
         2. Planning writes this technical plan and the goal brief.\n\
         3. Decomposition writes a task graph.\n\
         4. Verification design writes a test specification.\n\
         5. Local execution runs verification gates and refreshes proof evidence.\n\
         6. Review records controller review and security evidence.\n\
         7. Proof writes an honest not-ready proof until required execution, review, and integration evidence exists.\n\n\
         ## Execution Boundary\n\n\
         The current controller records planning task evidence, local verification task evidence, a bounded Wire-backed `goal-agent-execute` wave, agent mutation diff/changed-file evidence, post-mutation gate reruns when files change, and controller review/security evidence. The initial wave can make minimal project changes under the worker boundary, but readiness still requires specialist review loops and integration acceptance.\n\n\
         Goal ID: `{}`\n",
        state.goal_id
    );
    write_text_artifact(&state.state_dir.join(GOAL_TECHNICAL_PLAN_FILE), &body).await
}

async fn write_test_spec(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
) -> Result<()> {
    let task_lines = task_graph
        .tasks
        .iter()
        .map(|task| format!("- `{}`: {}", task.id, task.acceptance.join("; ")))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "# Test Spec\n\n\
         Generated: {generated_at}\n\n\
         ## Required Proof Before Ready\n\n\
         - Required gates must pass.\n\
         - A proof artifact must cite gate evidence.\n\
         - Known gaps must be empty or explicitly accepted by a human.\n\n\
         ## Scaffold Task Acceptance\n\n\
         {task_lines}\n\n\
         ## Current Status\n\n\
         `omk goal` remains `not_ready` because readiness still requires integration acceptance and specialist review loops beyond controller-owned planning, local verification, bounded agent execution, mutation evidence, post-mutation gate reruns, and controller review/security evidence.\n\n\
         Goal ID: `{}`\n",
        state.goal_id
    );
    write_text_artifact(&state.state_dir.join(GOAL_TEST_SPEC_FILE), &body).await
}

async fn write_task_graph(state: &GoalState, generated_at: DateTime<Utc>) -> Result<GoalTaskGraph> {
    let graph = GoalTaskGraph {
        version: 1,
        goal_id: state.goal_id.clone(),
        generated_at,
        tasks: vec![
            GoalTask {
                id: "goal-intake".to_string(),
                title: "Clarify durable goal intent".to_string(),
                description: "Preserve the original request, normalized goal, assumptions, and current execution boundary.".to_string(),
                status: GoalTaskStatus::Done,
                owner_role: Some(GOAL_CONTROLLER_ACTOR.to_string()),
                completed_at: Some(generated_at),
                evidence: vec![GoalTaskEvidence {
                    kind: "artifact".to_string(),
                    path: PathBuf::from(GOAL_PRD_FILE),
                    summary: "Goal brief records the original and normalized goal.".to_string(),
                }],
                dependencies: Vec::new(),
                read_set: vec!["goal.json".to_string()],
                write_set: vec![GOAL_PRD_FILE.to_string()],
                risk: "low".to_string(),
                acceptance: vec![
                    "goal brief exists".to_string(),
                    "original and normalized goals are recorded".to_string(),
                ],
            },
            GoalTask {
                id: "goal-plan".to_string(),
                title: "Design controller work plan".to_string(),
                description: "Persist a technical plan and explicit verification expectations for the goal controller.".to_string(),
                status: GoalTaskStatus::Done,
                owner_role: Some(GOAL_CONTROLLER_ACTOR.to_string()),
                completed_at: Some(generated_at),
                evidence: vec![
                    GoalTaskEvidence {
                        kind: "artifact".to_string(),
                        path: PathBuf::from(GOAL_TECHNICAL_PLAN_FILE),
                        summary: "Technical plan records controller phases and execution boundary.".to_string(),
                    },
                    GoalTaskEvidence {
                        kind: "artifact".to_string(),
                        path: PathBuf::from(GOAL_TASK_GRAPH_FILE),
                        summary: "Task graph records the first controller-owned work graph.".to_string(),
                    },
                    GoalTaskEvidence {
                        kind: "artifact".to_string(),
                        path: PathBuf::from(GOAL_TEST_SPEC_FILE),
                        summary: "Test spec records readiness expectations.".to_string(),
                    },
                ],
                dependencies: vec!["goal-intake".to_string()],
                read_set: vec![GOAL_PRD_FILE.to_string()],
                write_set: vec![
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                ],
                risk: "medium".to_string(),
                acceptance: vec![
                    "technical plan exists".to_string(),
                    "test spec describes readiness gates".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_LOCAL_VERIFY_TASK_ID.to_string(),
                title: "Run local verification wall".to_string(),
                description: "Run configured local gates, capture gate evidence, and refresh the goal proof.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec!["goal-plan".to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                    ".omk/gates.toml".to_string(),
                ],
                write_set: vec![
                    format!("{GOAL_ARTIFACTS_DIR}/{GOAL_GATE_ARTIFACTS_DIR}"),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "medium".to_string(),
                acceptance: vec![
                    "all required gates pass".to_string(),
                    "proof cites gate evidence".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_AGENT_EXECUTE_TASK_ID.to_string(),
                title: "Execute agent-owned implementation tasks".to_string(),
                description: "Run bounded agent work through Wire, allow minimal project mutations, and capture mutation evidence before readiness can be claimed.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec![GOAL_LOCAL_VERIFY_TASK_ID.to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                ],
                write_set: vec![
                    "project files".to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "high".to_string(),
                acceptance: vec![
                    "agent execution produces task and mutation evidence".to_string(),
                    "proof status becomes ready only with evidence".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_REVIEW_TASK_ID.to_string(),
                title: "Review goal execution evidence".to_string(),
                description: "Check that local verification and agent execution evidence are present before any readiness claim.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec![GOAL_AGENT_EXECUTE_TASK_ID.to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                    format!("{GOAL_ARTIFACTS_DIR}/{GOAL_AGENT_RUNS_DIR}"),
                ],
                write_set: vec![
                    format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_REVIEW_FILE}"),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "medium".to_string(),
                acceptance: vec![
                    "review artifact cites task and gate evidence".to_string(),
                    "review task remains blocked when execution evidence is missing".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_SECURITY_REVIEW_TASK_ID.to_string(),
                title: "Run security evidence check".to_string(),
                description: "Run a bounded controller security review over goal evidence and changed files.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec![GOAL_AGENT_EXECUTE_TASK_ID.to_string()],
                read_set: vec![
                    GOAL_PROOF_FILE.to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    "changed files".to_string(),
                ],
                write_set: vec![
                    format!(
                        "{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_SECURITY_REVIEW_FILE}"
                    ),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "high".to_string(),
                acceptance: vec![
                    "security review artifact exists".to_string(),
                    "high-confidence secret findings block the task".to_string(),
                ],
            },
        ],
    };
    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &graph).await?;
    Ok(graph)
}

async fn append_controller_task_events(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let mut events = Vec::new();

    for task in task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Done)
    {
        let task_id = TaskId(task.id.clone());
        events.push(
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "title": task.title,
                }))?,
        );
        events.push(builder.task_completed(
            task_id,
            worker_id.clone(),
            Some(&controller_task_summary(task)),
        )?);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}

fn controller_task_summary(task: &GoalTask) -> String {
    let artifacts = task
        .evidence
        .iter()
        .map(|evidence| evidence.path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} completed with artifact evidence: {}",
        task.id, artifacts
    )
}

fn apply_local_verification_task_result(
    task_graph: &mut GoalTaskGraph,
    gates: &[GateResult],
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let gates_ok = !gates.is_empty() && gates_passed(gates);
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID)?;

    task.status = if gates_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = gates_ok.then_some(completed_at);
    task.evidence = local_verification_task_evidence(gates, gates_ok);
    Some(task.clone())
}

fn local_verification_task_evidence(gates: &[GateResult], gates_ok: bool) -> Vec<GoalTaskEvidence> {
    let passed = gates.iter().filter(|gate| gate.passed).count();
    let gate_summary = if gates_ok {
        format!(
            "Local verification passed: {passed}/{} gate(s) succeeded.",
            gates.len()
        )
    } else if gates.is_empty() {
        "Local verification found no configured gates.".to_string()
    } else {
        format!(
            "Local verification is blocked: {passed}/{} gate(s) succeeded.",
            gates.len()
        )
    };

    vec![
        GoalTaskEvidence {
            kind: "gate_artifacts".to_string(),
            path: PathBuf::from(GOAL_ARTIFACTS_DIR).join(GOAL_GATE_ARTIFACTS_DIR),
            summary: gate_summary,
        },
        GoalTaskEvidence {
            kind: "proof".to_string(),
            path: PathBuf::from(GOAL_PROOF_FILE),
            summary: "Goal proof refreshed from local verification evidence.".to_string(),
        },
    ]
}

async fn append_local_verification_task_events(state: &GoalState, task: &GoalTask) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let task_id = TaskId(task.id.clone());
    let summary = controller_task_summary(task);

    let started = Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
        .with_actor(GOAL_CONTROLLER_ACTOR)
        .with_payload(serde_json::json!({
            "task_id": task.id,
            "worker_id": GOAL_CONTROLLER_ACTOR,
            "title": task.title,
        }))?;

    let finished = if task.status == GoalTaskStatus::Done {
        builder.task_completed(task_id, worker_id, Some(&summary))?
    } else {
        Event::new(RunId(state.goal_id.clone()), EventKind::TaskFailed)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": GOAL_CONTROLLER_ACTOR,
                "summary": summary,
            }))?
    };

    writer.append_many(&[started, finished]).await
}

async fn detect_git_evidence(project_dir: &Path) -> Option<GoalGitEvidence> {
    let inside = git_stdout(project_dir, &["rev-parse", "--is-inside-work-tree"]).await?;
    if inside != "true" {
        return None;
    }
    let branch = git_stdout(project_dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
    let head = git_stdout(project_dir, &["rev-parse", "HEAD"]).await?;
    let status = git_stdout(project_dir, &["status", "--porcelain"]).await?;

    Some(GoalGitEvidence {
        branch,
        head,
        dirty: !status.is_empty(),
    })
}

async fn git_stdout(project_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_dir)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn build_scaffold_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    git: Option<GoalGitEvidence>,
    generated_at: DateTime<Utc>,
) -> GoalProof {
    let commits = proof_commits(&git);
    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: "not ready: controller scaffold has not executed agents or verification gates"
            .to_string(),
        summary: format!(
            "Goal '{}' has durable planning artifacts, but no local verification or agent execution evidence yet.",
            state.normalized_goal
        ),
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        changed_files: Vec::new(),
        commits,
        git,
        gates: Vec::new(),
        post_mutation_gates_ran: false,
        known_gaps: vec![
            "agent execution has not run for this goal yet".to_string(),
            "verification gates have not run for this goal".to_string(),
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        ],
        human_decisions_required: Vec::new(),
    }
}

fn build_verified_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    gates: Vec<GateResult>,
    changed_files: Vec<String>,
    git: Option<GoalGitEvidence>,
    post_mutation_gates_ran: bool,
    generated_at: DateTime<Utc>,
) -> GoalProof {
    let gates_ok = !gates.is_empty() && gates_passed(&gates);
    let agent_execution_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_AGENT_EXECUTE_TASK_ID && task.status == GoalTaskStatus::Done);
    let review_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_REVIEW_TASK_ID && task.status == GoalTaskStatus::Done);
    let security_review_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_SECURITY_REVIEW_TASK_ID && task.status == GoalTaskStatus::Done);
    let commits = proof_commits(&git);
    let mut known_gaps = Vec::new();
    if !agent_execution_done {
        known_gaps.push("agent execution has not run for this goal yet".to_string());
        known_gaps.push(
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        );
    }
    if agent_execution_done && !review_done {
        known_gaps.push("review evidence has not run for this goal yet".to_string());
    }
    if agent_execution_done && !security_review_done {
        known_gaps.push("security review evidence has not run for this goal yet".to_string());
    }
    if agent_execution_done && !changed_files.is_empty() && !post_mutation_gates_ran {
        known_gaps
            .push("verification gates have not rerun after agent execution changes".to_string());
    }
    if agent_execution_done && review_done && security_review_done && changed_files.is_empty() {
        known_gaps.push(
            "project mutation and integration loop has not produced changed-file evidence yet"
                .to_string(),
        );
    }
    if agent_execution_done && review_done && security_review_done && !changed_files.is_empty() {
        known_gaps.push(
            "integration loop has not committed, opened a PR, or accepted the agent changes yet"
                .to_string(),
        );
    }

    if gates.is_empty() {
        known_gaps.push("no verification gates were detected or configured".to_string());
    } else if !gates_ok {
        known_gaps.push("required verification gates failed".to_string());
    }

    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: if gates_ok
            && agent_execution_done
            && review_done
            && security_review_done
            && !changed_files.is_empty()
            && post_mutation_gates_ran
        {
            "not ready: agent changes passed verification, review, and security evidence, but integration acceptance is missing".to_string()
        } else if gates_ok
            && agent_execution_done
            && review_done
            && security_review_done
            && !changed_files.is_empty()
        {
            "not ready: agent changes exist, but verification and integration have not rerun after the mutation".to_string()
        } else if gates_ok && agent_execution_done && review_done && security_review_done {
            "not ready: verification, agent execution, review, and security evidence passed, but no project mutation was captured".to_string()
        } else if gates_ok && agent_execution_done {
            "not ready: verification gates and bounded agent execution passed, but review/security evidence is missing".to_string()
        } else if gates_ok {
            "not ready: verification gates passed, but agent execution evidence is missing"
                .to_string()
        } else {
            "not ready: required verification evidence is incomplete or failing".to_string()
        },
        summary: format!(
            "Goal '{}' has {} gate result(s) and remains not ready until all required execution and review evidence exists.",
            state.normalized_goal,
            gates.len()
        ),
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        changed_files,
        commits,
        git,
        gates,
        post_mutation_gates_ran,
        known_gaps,
        human_decisions_required: Vec::new(),
    }
}

fn proof_commits(git: &Option<GoalGitEvidence>) -> Vec<String> {
    git.as_ref()
        .map(|evidence| vec![evidence.head.clone()])
        .unwrap_or_default()
}

async fn append_gate_events(state: &GoalState, gates: &[GateResult]) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let mut events = Vec::new();

    for gate in gates {
        let gate_id = GateId(gate.name.clone());
        events.push(builder.command_finished(
            gate_id.clone(),
            &gate.name,
            &gate.command_line,
            gate.exit_code,
            gate.timed_out,
            gate.stdout_summary.as_deref(),
            gate.stderr_summary.as_deref(),
            gate.output_path.as_deref(),
        )?);

        let gate_event = if gate.passed {
            builder.gate_passed_with_evidence(
                gate_id,
                &gate.name,
                gate.required,
                Some(&gate.command_line),
                gate.exit_code,
                gate.timed_out,
                gate.stdout_summary.as_deref(),
                gate.stderr_summary.as_deref(),
                gate.output_path.as_deref(),
                Some(gate.timeout_secs),
            )
        } else {
            builder.gate_failed_with_evidence(
                gate_id,
                &gate.name,
                gate.required,
                Some(&gate.command_line),
                gate.exit_code,
                gate.timed_out,
                gate.stdout_summary.as_deref(),
                gate.stderr_summary.as_deref(),
                gate.output_path.as_deref(),
                Some(gate.timeout_secs),
            )
        }?;
        events.push(gate_event);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}

fn summarize_task_graph(task_graph: &GoalTaskGraph) -> GoalTaskGraphSummary {
    GoalTaskGraphSummary {
        total_tasks: task_graph.tasks.len(),
        pending_tasks: task_graph
            .tasks
            .iter()
            .filter(|task| task.status == GoalTaskStatus::Pending)
            .count(),
        blocked_tasks: task_graph
            .tasks
            .iter()
            .filter(|task| task.status == GoalTaskStatus::Blocked)
            .count(),
        done_tasks: task_graph
            .tasks
            .iter()
            .filter(|task| task.status == GoalTaskStatus::Done)
            .count(),
    }
}

async fn write_text_artifact(path: &Path, body: &str) -> Result<()> {
    crate::runtime::atomic::atomic_write(path, body.as_bytes()).await
}

async fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    crate::runtime::atomic::atomic_write(path, json.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_status_serializes_as_snake_case() {
        let value = serde_json::to_value(GoalStatus::NotReady).unwrap();
        assert_eq!(value, "not_ready");
    }

    #[test]
    fn goal_phase_serializes_as_snake_case() {
        let value = serde_json::to_value(GoalPhase::VerificationDesign).unwrap();
        assert_eq!(value, "verification_design");
    }

    #[test]
    fn normalize_goal_collapses_whitespace() {
        assert_eq!(normalize_goal("  ship   it\nwell  "), "ship it well");
    }
}
