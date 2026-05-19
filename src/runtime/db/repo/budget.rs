use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::BudgetCheckpoint;

/// Operations on the `budget_checkpoints` table.
#[allow(async_fn_in_trait)]
pub trait BudgetRepo {
    async fn append_checkpoint(&self, checkpoint: &BudgetCheckpoint) -> Result<(), DbError>;
    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<BudgetCheckpoint>, DbError>;
    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct BudgetRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl BudgetRepo for BudgetRepoImpl {
    async fn append_checkpoint(&self, checkpoint: &BudgetCheckpoint) -> Result<(), DbError> {
        let checkpoint = checkpoint.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO budget_checkpoints (
                        goal_id, kind, limit_value, used_value, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        checkpoint.goal_id,
                        checkpoint.kind,
                        checkpoint.limit_value,
                        checkpoint.used_value,
                        checkpoint.created_at,
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<BudgetCheckpoint>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT
                        checkpoint_id, goal_id, kind, limit_value, used_value, created_at
                    FROM budget_checkpoints
                    WHERE goal_id = ?1
                    ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![goal_id], |row| {
                    Ok(BudgetCheckpoint {
                        checkpoint_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        kind: row.get(2)?,
                        limit_value: row.get(3)?,
                        used_value: row.get(4)?,
                        created_at: row.get(5)?,
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

    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM budget_checkpoints WHERE goal_id = ?1",
                    params![goal_id],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
