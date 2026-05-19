use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::TaskRecord;

/// Operations on the `tasks` table.
#[allow(async_fn_in_trait)]
pub trait TaskRepo {
    async fn create_batch(&self, goal_id: &str, tasks: &[TaskRecord]) -> Result<(), DbError>;
    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<TaskRecord>, DbError>;
    async fn update_status(&self, task_id: &str, status: &str) -> Result<(), DbError>;
    async fn update_task_graph(&self, goal_id: &str, tasks: &[TaskRecord]) -> Result<(), DbError>;
    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct TaskRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl TaskRepo for TaskRepoImpl {
    async fn create_batch(&self, goal_id: &str, tasks: &[TaskRecord]) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let tasks: Vec<TaskRecord> = tasks.to_vec();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "INSERT INTO tasks (
                        task_id, goal_id, kind, status, owner, write_set,
                        depends_on, retry_count, max_retries, lease_expires_at,
                        evidence_paths, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                )?;
                for task in &tasks {
                    stmt.execute(params![
                        task.task_id,
                        goal_id,
                        task.kind,
                        task.status,
                        task.owner,
                        task.write_set,
                        task.depends_on,
                        task.retry_count,
                        task.max_retries,
                        task.lease_expires_at,
                        task.evidence_paths,
                        task.created_at,
                        task.updated_at,
                    ])?;
                }
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<TaskRecord>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT
                        task_id, goal_id, kind, status, owner, write_set,
                        depends_on, retry_count, max_retries, lease_expires_at,
                        evidence_paths, created_at, updated_at
                    FROM tasks WHERE goal_id = ?1 ORDER BY created_at",
                )?;
                let rows = stmt.query_map(params![goal_id], |row| {
                    Ok(TaskRecord {
                        task_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        kind: row.get(2)?,
                        status: row.get(3)?,
                        owner: row.get(4)?,
                        write_set: row.get(5)?,
                        depends_on: row.get(6)?,
                        retry_count: row.get(7)?,
                        max_retries: row.get(8)?,
                        lease_expires_at: row.get(9)?,
                        evidence_paths: row.get(10)?,
                        created_at: row.get(11)?,
                        updated_at: row.get(12)?,
                    })
                })?;
                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn update_status(&self, task_id: &str, status: &str) -> Result<(), DbError> {
        let task_id = task_id.to_string();
        let status = status.to_string();
        let updated_at = chrono::Utc::now().timestamp();
        let task_id_for_err = task_id.clone();
        self.conn
            .call(move |conn| {
                let count = conn.execute(
                    "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE task_id = ?3",
                    params![status, updated_at, task_id],
                )?;
                if count == 0 {
                    return Err(tokio_rusqlite::Error::Rusqlite(
                        rusqlite::Error::QueryReturnedNoRows,
                    ));
                }
                Ok(())
            })
            .await
            .map_err(|e| match e {
                tokio_rusqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    DbError::TaskNotFound(task_id_for_err)
                }
                other => DbError::Connection(other),
            })
    }

    async fn update_task_graph(&self, goal_id: &str, tasks: &[TaskRecord]) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let tasks: Vec<TaskRecord> = tasks.to_vec();
        self.conn
            .call(move |conn| {
                conn.execute("DELETE FROM tasks WHERE goal_id = ?1", params![goal_id])?;
                let mut stmt = conn.prepare(
                    "INSERT INTO tasks (
                        task_id, goal_id, kind, status, owner, write_set,
                        depends_on, retry_count, max_retries, lease_expires_at,
                        evidence_paths, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                )?;
                for task in &tasks {
                    stmt.execute(params![
                        task.task_id,
                        goal_id,
                        task.kind,
                        task.status,
                        task.owner,
                        task.write_set,
                        task.depends_on,
                        task.retry_count,
                        task.max_retries,
                        task.lease_expires_at,
                        task.evidence_paths,
                        task.created_at,
                        task.updated_at,
                    ])?;
                }
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute("DELETE FROM tasks WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
