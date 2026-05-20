use anyhow::Result;
use chrono::Utc;
use crate::runtime::db::{
    error::DbError,
    repo::task::TaskRepo,
    types::TaskRecord,
    DbHandle,
};

use super::model::{GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

pub(crate) async fn try_load_from_db(goal_id: &str) -> Result<Option<GoalTaskGraph>, DbError> {
    let db = DbHandle::open(crate::runtime::goal::state::goals_db_path()).await?;
    try_load_from_db_with_handle(goal_id, &db).await
}

async fn try_load_from_db_with_handle(
    goal_id: &str,
    db: &DbHandle,
) -> Result<Option<GoalTaskGraph>, DbError> {
    let records = db.task_repo().get_by_goal(goal_id).await?;
    if records.is_empty() {
        return Ok(None);
    }

    let tasks: Result<Vec<_>, _> = records.into_iter().map(task_record_to_goal).collect();
    let tasks = tasks.map_err(|e| DbError::InvalidData(format!("task deserialization: {e}")))?;

    Ok(Some(GoalTaskGraph {
        version: 1,
        goal_id: goal_id.to_string(),
        generated_at: Utc::now(),
        tasks,
    }))
}

pub(crate) async fn try_save_to_db(graph: &GoalTaskGraph) -> Result<(), DbError> {
    let db = DbHandle::open(crate::runtime::goal::state::goals_db_path()).await?;
    try_save_to_db_with_handle(graph, &db).await
}

async fn try_save_to_db_with_handle(
    graph: &GoalTaskGraph,
    db: &DbHandle,
) -> Result<(), DbError> {
    let records: Vec<TaskRecord> = graph.tasks.iter().map(goal_task_to_record).collect();
    db.task_repo().update_task_graph(&graph.goal_id, &records).await
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn goal_task_to_record(task: &GoalTask) -> TaskRecord {
    TaskRecord {
        task_id: task.id.clone(),
        goal_id: String::new(), // filled by batch insert
        title: task.title.clone(),
        description: task.description.clone(),
        kind: "task".to_string(),
        status: task.status.to_string(),
        owner: task.owner_role.clone(),
        read_set: serde_json::to_string(&task.read_set).ok(),
        write_set: serde_json::to_string(&task.write_set).ok(),
        depends_on: serde_json::to_string(&task.dependencies).ok(),
        risk: task.risk.clone(),
        acceptance: serde_json::to_string(&task.acceptance).ok(),
        evidence: serde_json::to_string(&task.evidence).ok(),
        retry_count: task.retry_count as i32,
        max_retries: task.max_retries as i32,
        lease_expires_at: task.lease_expires_at.map(|dt| dt.timestamp()),
        completed_at: task.completed_at.map(|dt| dt.timestamp()),
        created_at: Utc::now().timestamp(),
        updated_at: Utc::now().timestamp(),
    }
}

fn task_record_to_goal(record: TaskRecord) -> Result<GoalTask> {
    let status = match record.status.as_str() {
        "pending" => GoalTaskStatus::Pending,
        "blocked" => GoalTaskStatus::Blocked,
        "done" => GoalTaskStatus::Done,
        other => anyhow::bail!("unknown task status: {other}"),
    };

    let parse_json_array = |s: Option<&str>| -> Vec<String> {
        s.and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
            .unwrap_or_default()
    };

    let parse_evidence = |s: Option<&str>| -> Vec<GoalTaskEvidence> {
        s.and_then(|json| serde_json::from_str::<Vec<GoalTaskEvidence>>(json).ok())
            .unwrap_or_default()
    };

    Ok(GoalTask {
        id: record.task_id,
        title: record.title,
        description: record.description,
        status,
        owner_role: record.owner,
        completed_at: record.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        evidence: parse_evidence(record.evidence.as_deref()),
        retry_count: record.retry_count as u32,
        max_retries: record.max_retries as u32,
        lease_expires_at: record
            .lease_expires_at
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        dependencies: parse_json_array(record.depends_on.as_deref()),
        read_set: parse_json_array(record.read_set.as_deref()),
        write_set: parse_json_array(record.write_set.as_deref()),
        risk: record.risk,
        acceptance: parse_json_array(record.acceptance.as_deref()),
    })
}

#[cfg(test)]
mod tests {
    use crate::runtime::db::repo::goal::GoalRepo;
    use crate::runtime::goal::task_graph::model::{
        GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus,
    };

    fn sample_task(id: &str) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status: GoalTaskStatus::Pending,
            owner_role: None,
            completed_at: None,
            evidence: vec![GoalTaskEvidence {
                kind: "test".to_string(),
                path: std::path::PathBuf::from("evidence.txt"),
                summary: "evidence".to_string(),
            }],
            retry_count: 0,
            max_retries: 3,
            lease_expires_at: None,
            dependencies: vec![],
            read_set: vec!["src/lib.rs".to_string()],
            write_set: vec!["src/lib.rs".to_string()],
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    #[tokio::test]
    async fn task_graph_db_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = crate::runtime::db::DbHandle::open(&db_path).await.unwrap();

        let goal_id = "goal-db-graph";
        let graph = GoalTaskGraph {
            version: 1,
            goal_id: goal_id.to_string(),
            generated_at: chrono::Utc::now(),
            tasks: vec![sample_task("task-a"), sample_task("task-b")],
        };

        // Pre-create the parent goal row so the FK constraint is satisfied.
        let goal_record = crate::runtime::db::types::GoalRecord {
            goal_id: goal_id.to_string(),
            status: "not_ready".to_string(),
            phase: "planning".to_string(),
            kind: None,
            original_goal: "test".to_string(),
            normalized_goal: "test".to_string(),
            goal_text: "test".to_string(),
            project_dir: "/tmp".to_string(),
            state_dir: "/tmp".to_string(),
            policy: "local".to_string(),
            delivery_policy: "local".to_string(),
            merge_policy: "disabled".to_string(),
            until_ready: false,
            slice_execution: false,
            max_agents: None,
            budget_time_secs: None,
            budget_tokens: None,
            budget_usd: None,
            cost_tracker_path: None,
            terminal_criteria: None,
            failure: None,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
            completed_at: None,
            controller_pid: None,
            version: 1,
        };
        db.goal_repo().create(&goal_record).await.unwrap();

        super::try_save_to_db_with_handle(&graph, &db).await.unwrap();

        let loaded = super::try_load_from_db_with_handle(goal_id, &db)
            .await
            .unwrap()
            .expect("graph should exist in DB");

        assert_eq!(loaded.goal_id, graph.goal_id);
        assert_eq!(loaded.tasks.len(), 2);
        assert_eq!(loaded.tasks[0].id, "task-a");
        assert_eq!(loaded.tasks[1].acceptance, vec!["Task task-b acceptance"]);
    }

    #[tokio::test]
    async fn task_graph_load_falls_back_to_json() {
        let tmp = tempfile::tempdir().unwrap();
        let goal_dir = tmp.path().join("goal-json-only");
        tokio::fs::create_dir_all(&goal_dir).await.unwrap();

        let graph = GoalTaskGraph {
            version: 1,
            goal_id: "goal-json-only".to_string(),
            generated_at: chrono::Utc::now(),
            tasks: vec![sample_task("task-1")],
        };
        let json = serde_json::to_vec_pretty(&graph).unwrap();
        tokio::fs::write(goal_dir.join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE), &json)
            .await
            .unwrap();

        let loaded = GoalTaskGraph::load(&goal_dir).await.unwrap();
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].id, "task-1");
    }
}
