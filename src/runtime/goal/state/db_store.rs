use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::runtime::db::{
    handle::DbHandle, repo::goal::GoalRepo, types::GoalRecord, DbError,
};

use super::store::GoalStateStore;
use super::types::GoalState;

/// SQLite-backed implementation of [`GoalStateStore`].
///
/// Persists goal state to a single SQLite database file rather than
/// per-goal JSON files. The `state_dir` field is still materialised on
/// load so that downstream code can locate artifact directories.
#[derive(Debug, Clone)]
pub struct DbGoalStateStore {
    db: DbHandle,
}

impl DbGoalStateStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

impl GoalStateStore for DbGoalStateStore {
    async fn save(&self, state: &GoalState) -> Result<()> {
        let record = goal_state_to_record(state)?;
        self.db.goal_repo().create(&record).await.map_err(map_db_err)
    }

    async fn load(&self, goal_dir: &Path) -> Result<GoalState> {
        let goal_id = goal_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if goal_id.is_empty() {
            anyhow::bail!("goal_dir has no valid goal_id: {}", goal_dir.display());
        }

        let record = self
            .db
            .goal_repo()
            .get(&goal_id)
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| super::GoalStateError::MissingFile {
                path: goal_dir.display().to_string(),
            })?;

        let mut state = record_to_goal_state(record)?;
        state.state_dir = goal_dir.to_path_buf();
        Ok(state)
    }

    async fn list(&self) -> Result<Vec<GoalState>> {
        let summaries = self
            .db
            .goal_repo()
            .list(crate::runtime::db::types::GoalFilter::default())
            .await
            .map_err(map_db_err)?;

        let mut goals = Vec::new();
        for summary in summaries {
            let goal_dir = super::persistence::goals_dir().join(&summary.goal_id);
            match self.load(&goal_dir).await {
                Ok(state) => goals.push(state),
                Err(error) => {
                    tracing::warn!(
                        goal_id = %summary.goal_id,
                        error = %error,
                        "Skipping unreadable goal state"
                    );
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
}

fn map_db_err(e: DbError) -> anyhow::Error {
    anyhow::anyhow!("db error: {e}")
}

fn goal_state_to_record(state: &GoalState) -> Result<GoalRecord> {
    Ok(GoalRecord {
        goal_id: state.goal_id.clone(),
        status: state.status.to_string(),
        phase: state.phase.to_string(),
        kind: None,
        original_goal: state.original_goal.clone(),
        normalized_goal: state.normalized_goal.clone(),
        goal_text: state.original_goal.clone(),
        project_dir: state.state_dir.display().to_string(),
        state_dir: state.state_dir.display().to_string(),
        policy: "local".to_string(),
        delivery_policy: state.delivery_policy.as_str().to_string(),
        merge_policy: state.merge_policy.as_str().to_string(),
        until_ready: state.until_ready,
        slice_execution: state.slice_execution,
        max_agents: state.max_agents.map(|v| v as i32),
        budget_time: state.budget_time.clone(),
        budget_tokens: state.budget_tokens.map(|v| v as i64),
        budget_usd: state.budget_usd.map(|v| (v * 100.0) as i64),
        cost_tracker_path: state.cost_tracker_path.as_ref().map(|p| p.display().to_string()),
        terminal_criteria: Some(serde_json::to_string(&state.terminal_criteria)?),
        failure: state
            .failure
            .as_ref()
            .map(|f| serde_json::to_string(f))
            .transpose()?,
        created_at: state.created_at.timestamp(),
        updated_at: state.updated_at.timestamp(),
        completed_at: state.completed_at.map(|dt| dt.timestamp()),
        controller_pid: None,
        version: state.version as i32,
    })
}

fn record_to_goal_state(record: GoalRecord) -> Result<GoalState> {
    let status = parse_goal_status(&record.status)?;
    let phase = parse_goal_phase(&record.phase)?;
    let delivery_policy = parse_delivery_policy(&record.delivery_policy)?;
    let merge_policy = parse_merge_policy(&record.merge_policy)?;

    let terminal_criteria = record
        .terminal_criteria
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to parse terminal_criteria, using default");
            Some(Default::default())
        })
        .unwrap_or_default();

    let failure = record
        .failure
        .map(|s| serde_json::from_str(&s))
        .transpose()?;

    Ok(GoalState {
        version: record.version as u32,
        goal_id: record.goal_id,
        original_goal: record.original_goal,
        normalized_goal: record.normalized_goal,
        status,
        phase,
        created_at: chrono::DateTime::from_timestamp(record.created_at, 0)
            .unwrap_or_else(|| chrono::Utc::now()),
        updated_at: chrono::DateTime::from_timestamp(record.updated_at, 0)
            .unwrap_or_else(|| chrono::Utc::now()),
        completed_at: record.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        until_ready: record.until_ready,
        budget_time: record.budget_time,
        budget_tokens: record.budget_tokens.map(|v| v as u64),
        budget_usd: record.budget_usd.map(|v| v as f64 / 100.0),
        max_agents: record.max_agents.map(|v| v as usize),
        cost_tracker_path: record.cost_tracker_path.map(PathBuf::from),
        terminal_criteria,
        delivery_policy,
        merge_policy,
        slice_execution: record.slice_execution,
        artifacts: Vec::new(),
        failure,
        state_dir: PathBuf::from(record.state_dir),
    })
}

fn parse_goal_status(s: &str) -> Result<super::types::GoalStatus> {
    match s {
        "running" => Ok(super::types::GoalStatus::Running),
        "ready" => Ok(super::types::GoalStatus::Ready),
        "not_ready" => Ok(super::types::GoalStatus::NotReady),
        "blocked_on_human" => Ok(super::types::GoalStatus::BlockedOnHuman),
        "blocked_on_external" => Ok(super::types::GoalStatus::BlockedOnExternal),
        "needs_more_budget" => Ok(super::types::GoalStatus::NeedsMoreBudget),
        "failed_infra" => Ok(super::types::GoalStatus::FailedInfra),
        "paused" => Ok(super::types::GoalStatus::Paused),
        "cancelled" => Ok(super::types::GoalStatus::Cancelled),
        _ => anyhow::bail!("unknown goal status: {s}"),
    }
}

fn parse_goal_phase(s: &str) -> Result<super::types::GoalPhase> {
    match s {
        "intake" => Ok(super::types::GoalPhase::Intake),
        "planning" => Ok(super::types::GoalPhase::Planning),
        "decomposition" => Ok(super::types::GoalPhase::Decomposition),
        "execution" => Ok(super::types::GoalPhase::Execution),
        "verification_design" => Ok(super::types::GoalPhase::VerificationDesign),
        "proof" => Ok(super::types::GoalPhase::Proof),
        _ => anyhow::bail!("unknown goal phase: {s}"),
    }
}

fn parse_delivery_policy(s: &str) -> Result<super::super::GoalDeliveryPolicy> {
    match s {
        "local" => Ok(super::super::GoalDeliveryPolicy::Local),
        "draft_pr" => Ok(super::super::GoalDeliveryPolicy::DraftPr),
        "auto_pr" => Ok(super::super::GoalDeliveryPolicy::AutoPr),
        _ => anyhow::bail!("unknown delivery policy: {s}"),
    }
}

fn parse_merge_policy(s: &str) -> Result<super::super::GoalMergePolicy> {
    match s {
        "disabled" => Ok(super::super::GoalMergePolicy::Disabled),
        "manual" => Ok(super::super::GoalMergePolicy::Manual),
        "gated" => Ok(super::super::GoalMergePolicy::Gated),
        _ => anyhow::bail!("unknown merge policy: {s}"),
    }
}

// ---------------------------------------------------------------------------
// GoalProof persistence helpers
// ---------------------------------------------------------------------------

pub async fn save_proof_to_db(db: &DbHandle, proof: &super::super::proof::GoalProof) -> Result<()> {
    use crate::runtime::db::ProofRepo;
    use crate::runtime::db::types::ProofRecord;
    use crate::runtime::goal::proof::sidecar;

    let delivery_metadata = sidecar::remembered_goal_proof_delivery_metadata(proof);
    let review_artifacts = sidecar::remembered_goal_proof_review_artifacts(proof);
    let integration_evidence = sidecar::remembered_goal_proof_integration_evidence(proof);
    let oracle_evidence = sidecar::remembered_goal_proof_oracle_evidence(proof);

    let record = ProofRecord {
        goal_id: proof.goal_id.clone(),
        version: proof.version as i32,
        status: proof.status.to_string(),
        readiness: proof.readiness.clone(),
        summary: proof.summary.clone(),
        task_graph_summary: Some(serde_json::to_string(&proof.task_graph_summary)?),
        changed_files: Some(serde_json::to_string(&proof.changed_files)?),
        commits: Some(serde_json::to_string(&proof.commits)?),
        git: proof.git.as_ref().map(|g| serde_json::to_string(g)).transpose()?,
        gates: Some(serde_json::to_string(&proof.gates)?),
        gates_passed: proof.gates.iter().filter(|g| g.passed).count() as i32,
        gates_total: proof.gates.len() as i32,
        post_mutation_gates_ran: proof.post_mutation_gates_ran,
        known_gaps: Some(serde_json::to_string(&proof.known_gaps)?),
        human_decisions_required: Some(serde_json::to_string(&proof.human_decisions_required)?),
        recovery_status: proof.recovery_status.clone(),
        delivery_metadata: delivery_metadata.as_ref().map(|v| serde_json::to_string(v)).transpose()?,
        review_artifacts: review_artifacts.as_ref().map(|v| serde_json::to_string(v)).transpose()?,
        integration_evidence: integration_evidence.as_ref().map(|v| serde_json::to_string(v)).transpose()?,
        oracle_evidence: oracle_evidence.as_ref().map(|v| serde_json::to_string(v)).transpose()?,
        generated_at: proof.generated_at.timestamp(),
    };

    db.proof_repo().upsert(&record).await.map_err(map_db_err)?;
    Ok(())
}

pub async fn load_proof_from_db(
    db: &DbHandle,
    goal_id: &str,
) -> Result<Option<super::super::proof::GoalProof>> {
    use crate::runtime::db::ProofRepo;
    use crate::runtime::db::types::ProofRecord;
    use crate::runtime::goal::proof::sidecar;

    let record: Option<ProofRecord> = db
        .proof_repo()
        .get(goal_id)
        .await
        .map_err(map_db_err)?;

    let Some(record) = record else {
        return Ok(None);
    };

    let task_graph_summary: super::super::task_graph::GoalTaskGraphSummary = record
        .task_graph_summary
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();

    let changed_files: Vec<String> = record
        .changed_files
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();

    let commits: Vec<String> = record
        .commits
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();

    let git: Option<super::super::evidence::GoalGitEvidence> = record
        .git
        .map(|s| serde_json::from_str(&s))
        .transpose()?;

    let gates: Vec<crate::runtime::gates::GateResult> = record
        .gates
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();

    let known_gaps: Vec<String> = record
        .known_gaps
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();

    let human_decisions_required: Vec<String> = record
        .human_decisions_required
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();

    let mut proof = super::super::proof::GoalProof {
        version: record.version as u32,
        goal_id: record.goal_id,
        status: parse_goal_status(&record.status)?,
        readiness: record.readiness,
        summary: record.summary,
        generated_at: chrono::DateTime::from_timestamp(record.generated_at, 0)
            .unwrap_or_else(|| chrono::Utc::now()),
        artifacts: Vec::new(),
        task_graph_summary,
        changed_files,
        commits,
        git,
        gates,
        post_mutation_gates_ran: record.post_mutation_gates_ran,
        known_gaps,
        human_decisions_required,
        recovery_status: record.recovery_status,
    };

    if let Some(dm) = record.delivery_metadata {
        let values: Vec<serde_json::Value> = serde_json::from_str(&dm)?;
        sidecar::remember_goal_proof_delivery_metadata(&proof, values);
    }
    if let Some(ra) = record.review_artifacts {
        let values: Vec<serde_json::Value> = serde_json::from_str(&ra)?;
        sidecar::remember_goal_proof_review_artifacts(&proof, values);
    }
    if let Some(ie) = record.integration_evidence {
        let value: serde_json::Value = serde_json::from_str(&ie)?;
        sidecar::remember_goal_proof_acceptance_evidence(&proof, value, serde_json::Value::Null);
    }
    if let Some(oe) = record.oracle_evidence {
        let value: serde_json::Value = serde_json::from_str(&oe)?;
        sidecar::remember_goal_proof_acceptance_evidence(&proof, serde_json::Value::Null, value);
    }

    Ok(Some(proof))
}

// ---------------------------------------------------------------------------
// GoalTaskGraph persistence helpers
// ---------------------------------------------------------------------------

pub async fn save_task_graph_to_db(
    db: &DbHandle,
    graph: &super::super::task_graph::GoalTaskGraph,
) -> Result<()> {
    use crate::runtime::db::TaskRepo;
    use crate::runtime::db::types::TaskRecord;

    let tasks: Vec<TaskRecord> = graph
        .tasks
        .iter()
        .map(|t| task_to_record(t, &graph.goal_id))
        .collect::<Result<Vec<_>>>()?;

    db.task_repo()
        .update_task_graph(&graph.goal_id, &tasks)
        .await
        .map_err(map_db_err)?;
    Ok(())
}

pub async fn load_task_graph_from_db(
    db: &DbHandle,
    goal_id: &str,
) -> Result<Option<super::super::task_graph::GoalTaskGraph>> {
    use crate::runtime::db::TaskRepo;

    let records = db
        .task_repo()
        .get_by_goal(goal_id)
        .await
        .map_err(map_db_err)?;

    if records.is_empty() {
        return Ok(None);
    }

    let tasks: Vec<super::super::task_graph::GoalTask> = records
        .into_iter()
        .map(record_to_task)
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(super::super::task_graph::GoalTaskGraph {
        version: 1,
        goal_id: goal_id.to_string(),
        generated_at: chrono::Utc::now(),
        tasks,
    }))
}

fn task_to_record(
    task: &super::super::task_graph::GoalTask,
    goal_id: &str,
) -> Result<crate::runtime::db::types::TaskRecord> {
    Ok(crate::runtime::db::types::TaskRecord {
        task_id: task.id.clone(),
        goal_id: goal_id.to_string(),
        title: task.title.clone(),
        description: task.description.clone(),
        kind: "task".to_string(),
        status: task.status.to_string(),
        owner: task.owner_role.clone(),
        read_set: Some(serde_json::to_string(&task.read_set)?),
        write_set: Some(serde_json::to_string(&task.write_set)?),
        depends_on: Some(serde_json::to_string(&task.dependencies)?),
        risk: task.risk.clone(),
        acceptance: Some(serde_json::to_string(&task.acceptance)?),
        evidence: Some(serde_json::to_string(&task.evidence)?),
        retry_count: task.retry_count as i32,
        max_retries: task.max_retries as i32,
        lease_expires_at: task.lease_expires_at.map(|dt| dt.timestamp()),
        completed_at: task.completed_at.map(|dt| dt.timestamp()),
        created_at: 0,
        updated_at: 0,
    })
}

fn record_to_task(
    record: crate::runtime::db::types::TaskRecord,
) -> Result<super::super::task_graph::GoalTask> {
    Ok(super::super::task_graph::GoalTask {
        id: record.task_id,
        title: record.title,
        description: record.description,
        status: parse_task_status(&record.status)?,
        owner_role: record.owner,
        completed_at: record.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        evidence: record
            .evidence
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default(),
        retry_count: record.retry_count as u32,
        max_retries: record.max_retries as u32,
        lease_expires_at: record.lease_expires_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        dependencies: record
            .depends_on
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default(),
        read_set: record
            .read_set
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default(),
        write_set: record
            .write_set
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default(),
        risk: record.risk,
        acceptance: record
            .acceptance
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default(),
    })
}

fn parse_task_status(s: &str) -> Result<super::super::task_graph::GoalTaskStatus> {
    match s {
        "pending" => Ok(super::super::task_graph::GoalTaskStatus::Pending),
        "blocked" => Ok(super::super::task_graph::GoalTaskStatus::Blocked),
        "done" => Ok(super::super::task_graph::GoalTaskStatus::Done),
        _ => anyhow::bail!("unknown task status: {s}"),
    }
}
