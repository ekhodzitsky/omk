use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
const GOAL_AGENT_WORKER_ID: &str = "goal-agent-worker-0";
const GOAL_AGENT_WORKER_ROLE: &str = "executor";

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
    worker_outbox_path: PathBuf,
    wire_events_path: PathBuf,
    worker_summary: Option<String>,
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

    let proof = build_verified_proof(&state, &task_graph, gates, changed_files, git, now);
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
        verification_proof.gates,
        verification_proof.changed_files,
        verification_proof.git,
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
    let scheduler_task = Task::new(GOAL_AGENT_EXECUTE_TASK_ID, "goal-agent-execute")
        .with_description(goal_agent_task_prompt(state, task_graph, started_at))
        .with_read_set(vec![
            GOAL_PRD_FILE.to_string(),
            GOAL_TECHNICAL_PLAN_FILE.to_string(),
            GOAL_TEST_SPEC_FILE.to_string(),
            GOAL_TASK_GRAPH_FILE.to_string(),
        ])
        .with_write_set(vec![GOAL_PROOF_FILE.to_string()])
        .with_max_retries(0);

    event_writer
        .append(&builder.run_started("goal-agent", project_dir, &scheduler_task.description)?)
        .await?;

    if !goal_agent_wire_runtime_available() {
        let summary = "Kimi CLI not found; install/authenticate kimi or set MOCK_KIMI to a mock binary before running goal agent execution";
        event_writer.append(&builder.run_failed(summary)?).await?;
        return Ok(GoalAgentRunEvidence {
            summary: RunSummary {
                run_id,
                completed: 0,
                failed: 1,
                cancelled: 0,
                total: 1,
            },
            run_path,
            worker_outbox_path,
            wire_events_path,
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
        vec![scheduler_task],
    )
    .await?;

    let run_result = runner.run(std::slice::from_ref(&spec)).await;
    cancel.cancel();
    stop_wire_worker(&mut handle).await;

    let summary = run_result?;
    let worker_summary = read_goal_agent_worker_summary(&spec).await?;

    Ok(GoalAgentRunEvidence {
        summary,
        run_path,
        worker_outbox_path,
        wire_events_path,
        worker_summary,
    })
}

fn goal_agent_task_prompt(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
) -> String {
    let local_status = task_graph
        .tasks
        .iter()
        .find(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID)
        .map(|task| task.status.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "Goal ID: {}\nGenerated: {generated_at}\n\nOriginal goal:\n{}\n\nNormalized goal:\n{}\n\nTask:\nReview the existing goal artifacts and produce a concise implementation execution note. Do not mutate project files in this bounded wave. Record what would be needed next for production readiness.\n\nLocal verification task status: {local_status}",
        state.goal_id, state.original_goal, state.normalized_goal
    )
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

async fn read_goal_agent_worker_summary(spec: &WorkerSpec) -> Result<Option<String>> {
    let results: Vec<WorkerResult> = spec.read_results().await?;
    Ok(results
        .into_iter()
        .find(|result| result.task_id == GOAL_AGENT_EXECUTE_TASK_ID)
        .map(|result| result.summary))
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

    vec![
        GoalTaskEvidence {
            kind: "agent_run".to_string(),
            path: evidence.run_path.clone(),
            summary: run_summary,
        },
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
    ]
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
         This scaffold captures intent and durable planning artifacts. Run `omk goal execute` to attach local gate evidence and a bounded Wire-backed agent wave.\n",
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
         6. Proof writes an honest not-ready proof until required execution and review evidence exists.\n\n\
         ## Execution Boundary\n\n\
         The current controller records planning task evidence, local verification task evidence, and a bounded Wire-backed `goal-agent-execute` wave. The initial wave records agent-owned execution evidence without mutating project files; future slices will add review loops, task mutation, and integration.\n\n\
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
         `omk goal` remains `not_ready` because readiness still requires review/security evidence beyond controller-owned planning, local verification, and the bounded agent execution wave.\n\n\
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
                description: "Run future agent work through the task graph, reviews, and integration before readiness can be claimed.".to_string(),
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
                write_set: vec![GOAL_PROOF_FILE.to_string()],
                risk: "high".to_string(),
                acceptance: vec![
                    "agent execution produces task evidence".to_string(),
                    "proof status becomes ready only with evidence".to_string(),
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
    generated_at: DateTime<Utc>,
) -> GoalProof {
    let gates_ok = !gates.is_empty() && gates_passed(&gates);
    let agent_execution_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_AGENT_EXECUTE_TASK_ID && task.status == GoalTaskStatus::Done);
    let commits = proof_commits(&git);
    let mut known_gaps = if agent_execution_done {
        vec![
            "review evidence is not implemented for omk goal yet".to_string(),
            "security and integration hardening evidence is not captured for omk goal yet"
                .to_string(),
        ]
    } else {
        vec![
            "agent execution has not run for this goal yet".to_string(),
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        ]
    };

    if gates.is_empty() {
        known_gaps.push("no verification gates were detected or configured".to_string());
    } else if !gates_ok {
        known_gaps.push("required verification gates failed".to_string());
    }

    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: if gates_ok && agent_execution_done {
            "not ready: verification gates and bounded agent execution passed, but review evidence is missing"
                .to_string()
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
