use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::TaskRecord;

/// Operations on the `tasks` table.
#[allow(async_fn_in_trait)]
pub trait TaskRepo {
    async fn create_batch(&self, goal_id: &str, tasks: &[TaskRecord]) -> Result<(), DbError>;
    async fn get_by_id(&self, task_id: &str) -> Result<Option<TaskRecord>, DbError>;
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
        if tasks.iter().any(|t| t.goal_id != goal_id) {
            return Err(DbError::InvalidData(format!(
                "create_batch: not all tasks belong to goal {}",
                goal_id
            )));
        }
        let goal_id = goal_id.to_string();
        let tasks: Vec<TaskRecord> = tasks.to_vec();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                let mut stmt = conn.prepare(
                    "INSERT INTO tasks (
                        task_id, goal_id, title, description, kind, status, owner,
                        read_set, write_set, depends_on, risk, acceptance, evidence,
                        retry_count, max_retries, lease_expires_at, completed_at,
                        created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                )?;
                for task in &tasks {
                    stmt.execute(params![
                        task.task_id,
                        goal_id,
                        task.title,
                        task.description,
                        task.kind,
                        task.status,
                        task.owner,
                        task.read_set,
                        task.write_set,
                        task.depends_on,
                        task.risk,
                        task.acceptance,
                        task.evidence,
                        task.retry_count,
                        task.max_retries,
                        task.lease_expires_at,
                        task.completed_at,
                        task.created_at,
                        task.updated_at,
                    ])?;
                }
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_id(&self, task_id: &str) -> Result<Option<TaskRecord>, DbError> {
        let task_id = task_id.to_string();
        self.conn
            .call(move |conn| -> Result<Option<TaskRecord>, rusqlite::Error> {
                let mut stmt = conn.prepare(
                    "SELECT
                        task_id, goal_id, title, description, kind, status, owner,
                        read_set, write_set, depends_on, risk, acceptance, evidence,
                        retry_count, max_retries, lease_expires_at, completed_at,
                        created_at, updated_at
                    FROM tasks WHERE task_id = ?1",
                )?;
                let mut rows = stmt.query(params![task_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(TaskRecord {
                        task_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        title: row.get(2)?,
                        description: row.get(3)?,
                        kind: row.get(4)?,
                        status: row.get(5)?,
                        owner: row.get(6)?,
                        read_set: row.get(7)?,
                        write_set: row.get(8)?,
                        depends_on: row.get(9)?,
                        risk: row.get(10)?,
                        acceptance: row.get(11)?,
                        evidence: row.get(12)?,
                        retry_count: row.get(13)?,
                        max_retries: row.get(14)?,
                        lease_expires_at: row.get(15)?,
                        completed_at: row.get(16)?,
                        created_at: row.get(17)?,
                        updated_at: row.get(18)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<TaskRecord>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| -> Result<Vec<TaskRecord>, rusqlite::Error> {
                let mut stmt = conn.prepare(
                    "SELECT
                        task_id, goal_id, title, description, kind, status, owner,
                        read_set, write_set, depends_on, risk, acceptance, evidence,
                        retry_count, max_retries, lease_expires_at, completed_at,
                        created_at, updated_at
                    FROM tasks WHERE goal_id = ?1 ORDER BY created_at",
                )?;
                let rows = stmt.query_map(params![goal_id], |row| {
                    Ok(TaskRecord {
                        task_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        title: row.get(2)?,
                        description: row.get(3)?,
                        kind: row.get(4)?,
                        status: row.get(5)?,
                        owner: row.get(6)?,
                        read_set: row.get(7)?,
                        write_set: row.get(8)?,
                        depends_on: row.get(9)?,
                        risk: row.get(10)?,
                        acceptance: row.get(11)?,
                        evidence: row.get(12)?,
                        retry_count: row.get(13)?,
                        max_retries: row.get(14)?,
                        lease_expires_at: row.get(15)?,
                        completed_at: row.get(16)?,
                        created_at: row.get(17)?,
                        updated_at: row.get(18)?,
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
        let count = self
            .conn
            .call(move |conn| -> Result<usize, rusqlite::Error> {
                conn.execute(
                    "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE task_id = ?3",
                    params![status, updated_at, task_id],
                )
            })
            .await
            .map_err(DbError::Connection)?;
        if count == 0 {
            return Err(DbError::TaskNotFound(task_id_for_err));
        }
        Ok(())
    }

    async fn update_task_graph(&self, goal_id: &str, tasks: &[TaskRecord]) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let tasks: Vec<TaskRecord> = tasks.to_vec();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute("DELETE FROM tasks WHERE goal_id = ?1", params![goal_id])?;
                let mut stmt = conn.prepare(
                    "INSERT INTO tasks (
                        task_id, goal_id, title, description, kind, status, owner,
                        read_set, write_set, depends_on, risk, acceptance, evidence,
                        retry_count, max_retries, lease_expires_at, completed_at,
                        created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                )?;
                for task in &tasks {
                    stmt.execute(params![
                        task.task_id,
                        goal_id,
                        task.title,
                        task.description,
                        task.kind,
                        task.status,
                        task.owner,
                        task.read_set,
                        task.write_set,
                        task.depends_on,
                        task.risk,
                        task.acceptance,
                        task.evidence,
                        task.retry_count,
                        task.max_retries,
                        task.lease_expires_at,
                        task.completed_at,
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
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute("DELETE FROM tasks WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
