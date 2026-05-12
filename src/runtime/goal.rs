use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, GateId, RunId};
use crate::runtime::gates::{
    detect_changed_files, gates_passed, load_or_detect_gates, run_gates_with_evidence, GateResult,
};

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
    VerificationDesign,
    Proof,
}

impl std::fmt::Display for GoalPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalPhase::Intake => "intake",
            GoalPhase::Planning => "planning",
            GoalPhase::Decomposition => "decomposition",
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: GoalTaskStatus,
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

    state.phase = GoalPhase::Proof;
    let proof = build_scaffold_proof(&state, &task_graph, now);
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;
    record_artifact(&mut state, "proof", GOAL_PROOF_FILE, now);

    state.status = GoalStatus::NotReady;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;

    writer
        .append(
            &builder.run_failed(
                "goal controller scaffold created; agent execution is not implemented",
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
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let gate_config = load_or_detect_gates(project_dir).await;
    let gate_artifacts = state
        .state_dir
        .join(GOAL_ARTIFACTS_DIR)
        .join(GOAL_GATE_ARTIFACTS_DIR);
    let gates = run_gates_with_evidence(&gate_config, project_dir, Some(&gate_artifacts)).await;
    let changed_files = detect_changed_files(project_dir).await;
    let now = Utc::now();

    append_gate_events(&state, &gates).await?;

    state.status = GoalStatus::NotReady;
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;

    let proof = build_verified_proof(&state, &task_graph, gates, changed_files, now);
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

async fn write_goal_brief(state: &GoalState, generated_at: DateTime<Utc>) -> Result<()> {
    let body = format!(
        "# Goal Brief\n\n\
         Generated: {generated_at}\n\n\
         ## Original Goal\n\n\
         {}\n\n\
         ## Normalized Goal\n\n\
         {}\n\n\
         ## Current Scope\n\n\
         This scaffold captures intent and durable planning artifacts. Agent execution is intentionally not implemented in this slice.\n",
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
         5. Proof writes an honest not-ready proof until execution is implemented.\n\n\
         ## Execution Boundary\n\n\
         The current controller does not launch agents, mutate project files, or run verification gates.\n\n\
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
         `omk goal` remains `not_ready` because the controller has not executed the task graph.\n\n\
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
                status: GoalTaskStatus::Pending,
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
                status: GoalTaskStatus::Pending,
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
                id: "goal-execute-verify".to_string(),
                title: "Execute, verify, and prove readiness".to_string(),
                description: "Run the future task graph through agents, gates, reviews, and proof generation.".to_string(),
                status: GoalTaskStatus::Pending,
                dependencies: vec!["goal-plan".to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                ],
                write_set: vec![GOAL_PROOF_FILE.to_string()],
                risk: "high".to_string(),
                acceptance: vec![
                    "all required gates pass".to_string(),
                    "proof status becomes ready only with evidence".to_string(),
                ],
            },
        ],
    };
    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &graph).await?;
    Ok(graph)
}

fn build_scaffold_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
) -> GoalProof {
    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: "not ready: controller scaffold has not executed agents or verification gates"
            .to_string(),
        summary: format!(
            "Goal '{}' has durable planning artifacts, but no execution evidence yet.",
            state.normalized_goal
        ),
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        changed_files: Vec::new(),
        commits: Vec::new(),
        gates: Vec::new(),
        known_gaps: vec![
            "agent execution is not implemented for omk goal yet".to_string(),
            "verification gates have not run for this goal".to_string(),
            "proof cannot claim readiness until task execution evidence exists".to_string(),
        ],
        human_decisions_required: Vec::new(),
    }
}

fn build_verified_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    gates: Vec<GateResult>,
    changed_files: Vec<String>,
    generated_at: DateTime<Utc>,
) -> GoalProof {
    let gates_ok = !gates.is_empty() && gates_passed(&gates);
    let mut known_gaps = vec![
        "agent execution is not implemented for omk goal yet".to_string(),
        "proof cannot claim readiness until task execution evidence exists".to_string(),
    ];

    if gates.is_empty() {
        known_gaps.push("no verification gates were detected or configured".to_string());
    } else if !gates_ok {
        known_gaps.push("required verification gates failed".to_string());
    }

    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: if gates_ok {
            "not ready: verification gates passed, but task execution evidence is missing"
                .to_string()
        } else {
            "not ready: required verification evidence is incomplete or failing".to_string()
        },
        summary: format!(
            "Goal '{}' has {} gate result(s) and remains not ready until execution evidence exists.",
            state.normalized_goal,
            gates.len()
        ),
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        changed_files,
        commits: Vec::new(),
        gates,
        known_gaps,
        human_decisions_required: Vec::new(),
    }
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
