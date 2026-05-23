use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::DecisionRecord;

/// Operations on the `decisions` table.
#[allow(async_fn_in_trait)]
pub trait DecisionRepo {
    async fn append(&self, decision: &DecisionRecord) -> Result<(), DbError>;
    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<DecisionRecord>, DbError>;
    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct DecisionRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl DecisionRepo for DecisionRepoImpl {
    async fn append(&self, decision: &DecisionRecord) -> Result<(), DbError> {
        let decision = decision.clone();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "INSERT INTO decisions (
                        goal_id, version, actor, phase, kind, decision,
                        rationale, constraints, artifacts, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        decision.goal_id,
                        decision.version,
                        decision.actor,
                        decision.phase,
                        decision.kind,
                        decision.decision,
                        decision.rationale,
                        decision.constraints,
                        decision.artifacts,
                        decision.created_at,
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_goal(&self, goal_id: &str) -> Result<Vec<DecisionRecord>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(
                move |conn| -> Result<Vec<DecisionRecord>, rusqlite::Error> {
                    let mut stmt = conn.prepare(
                        "SELECT
                        decision_id, goal_id, version, actor, phase, kind, decision,
                        rationale, constraints, artifacts, created_at
                    FROM decisions
                    WHERE goal_id = ?1
                    ORDER BY created_at ASC",
                    )?;
                    let rows = stmt.query_map(params![goal_id], |row| {
                        Ok(DecisionRecord {
                            decision_id: row.get(0)?,
                            goal_id: row.get(1)?,
                            version: row.get(2)?,
                            actor: row.get(3)?,
                            phase: row.get(4)?,
                            kind: row.get(5)?,
                            decision: row.get(6)?,
                            rationale: row.get(7)?,
                            constraints: row.get(8)?,
                            artifacts: row.get(9)?,
                            created_at: row.get(10)?,
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
                conn.execute("DELETE FROM decisions WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
