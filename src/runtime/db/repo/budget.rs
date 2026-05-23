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
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "INSERT INTO budget_checkpoints (
                        goal_id, version, label, status, phase, recorded_at,
                        budget_time, total_budget_secs, elapsed_since_created_secs,
                        remaining_budget_secs, budget_tokens, used_tokens,
                        remaining_budget_tokens, budget_usd, estimated_cost_usd,
                        remaining_budget_usd, limit_value, used_value, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                    params![
                        checkpoint.goal_id,
                        checkpoint.version,
                        checkpoint.label,
                        checkpoint.status,
                        checkpoint.phase,
                        checkpoint.recorded_at,
                        checkpoint.budget_time,
                        checkpoint.total_budget_secs,
                        checkpoint.elapsed_since_created_secs,
                        checkpoint.remaining_budget_secs,
                        checkpoint.budget_tokens,
                        checkpoint.used_tokens,
                        checkpoint.remaining_budget_tokens,
                        checkpoint.budget_usd,
                        checkpoint.estimated_cost_usd,
                        checkpoint.remaining_budget_usd,
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
            .call(
                move |conn| -> Result<Vec<BudgetCheckpoint>, rusqlite::Error> {
                    let mut stmt = conn.prepare(
                        "SELECT
                        checkpoint_id, goal_id, version, label, status, phase, recorded_at,
                        budget_time, total_budget_secs, elapsed_since_created_secs,
                        remaining_budget_secs, budget_tokens, used_tokens,
                        remaining_budget_tokens, budget_usd, estimated_cost_usd,
                        remaining_budget_usd, limit_value, used_value, created_at
                    FROM budget_checkpoints
                    WHERE goal_id = ?1
                    ORDER BY created_at ASC",
                    )?;
                    let rows = stmt.query_map(params![goal_id], |row| {
                        Ok(BudgetCheckpoint {
                            checkpoint_id: row.get(0)?,
                            goal_id: row.get(1)?,
                            version: row.get(2)?,
                            label: row.get(3)?,
                            status: row.get(4)?,
                            phase: row.get(5)?,
                            recorded_at: row.get(6)?,
                            budget_time: row.get(7)?,
                            total_budget_secs: row.get(8)?,
                            elapsed_since_created_secs: row.get(9)?,
                            remaining_budget_secs: row.get(10)?,
                            budget_tokens: row.get(11)?,
                            used_tokens: row.get(12)?,
                            remaining_budget_tokens: row.get(13)?,
                            budget_usd: row.get(14)?,
                            estimated_cost_usd: row.get(15)?,
                            remaining_budget_usd: row.get(16)?,
                            limit_value: row.get(17)?,
                            used_value: row.get(18)?,
                            created_at: row.get(19)?,
                        })
                    })?;
                    let mut results = Vec::new();
                    for row in rows {
                        results.push(row?);
                    }
                    Ok(results)
                },
            )
            .await
            .map_err(DbError::Connection)
    }

    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
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
