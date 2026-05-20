use super::state::{GoalPhase, GoalState, GOAL_DECISIONS_FILE};
use super::task_graph::GoalTaskGraph;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalDecisionRecord {
    pub(crate) version: u32,
    pub(crate) goal_id: String,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) actor: String,
    pub(crate) phase: GoalPhase,
    pub(crate) kind: String,
    pub(crate) decision: String,
    pub(crate) rationale: String,
    pub(crate) constraints: Vec<String>,
    pub(crate) artifacts: Vec<PathBuf>,
}

impl GoalDecisionRecord {
    fn controller(
        state: &GoalState,
        created_at: DateTime<Utc>,
        phase: GoalPhase,
        kind: &str,
        decision: &str,
        rationale: &str,
        artifacts: &[&str],
    ) -> Self {
        Self {
            version: 1,
            goal_id: state.goal_id.clone(),
            created_at,
            actor: super::state::GOAL_CONTROLLER_ACTOR.to_string(),
            phase,
            kind: kind.to_string(),
            decision: decision.to_string(),
            rationale: rationale.to_string(),
            constraints: vec![
                "local GitHub-only release lane".to_string(),
                "readiness requires proof-backed verification evidence".to_string(),
            ],
            artifacts: artifacts.iter().map(PathBuf::from).collect(),
        }
    }
}

pub(crate) async fn append_controller_scaffold_decisions(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    created_at: DateTime<Utc>,
) -> Result<()> {
    let task_count = task_graph.tasks.len();
    let decisions = vec![
        GoalDecisionRecord::controller(
            state,
            created_at,
            GoalPhase::Planning,
            "planning_boundary",
            "Create durable planning, task graph, and proof scaffolds before any agent work.",
            "Long-running goals need inspectable state before autonomy spends budget or mutates project files.",
            &[
                super::state::GOAL_PRD_FILE,
                super::state::GOAL_TECHNICAL_PLAN_FILE,
                super::state::GOAL_TEST_SPEC_FILE,
            ],
        ),
        GoalDecisionRecord::controller(
            state,
            created_at,
            GoalPhase::Decomposition,
            "task_graph_shape",
            &format!("Start with {task_count} controller-owned and pending execution/review tasks."),
            "The controller needs an explicit graph so future agents can be budgeted, ordered, and policy-validated.",
            &[super::state::GOAL_TASK_GRAPH_FILE],
        ),
        GoalDecisionRecord::controller(
            state,
            created_at,
            GoalPhase::Execution,
            "execution_boundary",
            "Keep readiness not_ready until local gates, bounded agent execution, and review evidence all exist.",
            "A generated plan is not the same as a production-ready result; proof must cite execution evidence.",
            &[
                super::state::GOAL_TASK_GRAPH_FILE,
                super::state::GOAL_PROOF_FILE,
            ],
        ),
    ];

    append_goal_decisions(state, &decisions).await
}

async fn append_goal_decisions(state: &GoalState, decisions: &[GoalDecisionRecord]) -> Result<()> {
    if let Some(db) = crate::runtime::db::global_db() {
        use crate::runtime::db::DecisionRepo;
        for decision in decisions {
            let record = decision_to_record(decision)?;
            db.decision_repo().append(&record).await.map_err(|e| anyhow::anyhow!("db error: {e}"))?;
        }
        return Ok(());
    }

    // Fallback to JSONL when global DB is not initialized.
    let mut buffer = Vec::new();
    for decision in decisions {
        serde_json::to_writer(&mut buffer, decision)?;
        buffer.push(b'\n');
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.state_dir.join(GOAL_DECISIONS_FILE))
        .await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, &buffer).await?;
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    Ok(())
}

fn decision_to_record(decision: &GoalDecisionRecord) -> Result<crate::runtime::db::types::DecisionRecord> {
    Ok(crate::runtime::db::types::DecisionRecord {
        decision_id: None,
        goal_id: decision.goal_id.clone(),
        version: decision.version as i32,
        actor: decision.actor.clone(),
        phase: decision.phase.to_string(),
        kind: decision.kind.clone(),
        decision: decision.decision.clone(),
        rationale: decision.rationale.clone(),
        constraints: Some(serde_json::to_string(&decision.constraints)?),
        artifacts: Some(serde_json::to_string(&decision.artifacts)?),
        created_at: decision.created_at.timestamp(),
    })
}
