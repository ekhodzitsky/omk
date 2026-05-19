use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::ProofRecord;

/// Operations on the `proofs` table.
#[allow(async_fn_in_trait)]
pub trait ProofRepo {
    async fn upsert(&self, proof: &ProofRecord) -> Result<(), DbError>;
    async fn get(&self, goal_id: &str) -> Result<Option<ProofRecord>, DbError>;
    async fn delete(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct ProofRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl ProofRepo for ProofRepoImpl {
    async fn upsert(&self, proof: &ProofRecord) -> Result<(), DbError> {
        let proof = proof.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO proofs (
                        goal_id, status, gates_passed, gates_total,
                        changed_files, known_gaps, recovery_status, generated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    ON CONFLICT(goal_id) DO UPDATE SET
                        status = excluded.status,
                        gates_passed = excluded.gates_passed,
                        gates_total = excluded.gates_total,
                        changed_files = excluded.changed_files,
                        known_gaps = excluded.known_gaps,
                        recovery_status = excluded.recovery_status,
                        generated_at = excluded.generated_at",
                    params![
                        proof.goal_id,
                        proof.status,
                        proof.gates_passed,
                        proof.gates_total,
                        proof.changed_files,
                        proof.known_gaps,
                        proof.recovery_status,
                        proof.generated_at,
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get(&self, goal_id: &str) -> Result<Option<ProofRecord>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT
                        goal_id, status, gates_passed, gates_total,
                        changed_files, known_gaps, recovery_status, generated_at
                    FROM proofs WHERE goal_id = ?1",
                )?;
                let mut rows = stmt.query(params![goal_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(ProofRecord {
                        goal_id: row.get(0)?,
                        status: row.get(1)?,
                        gates_passed: row.get(2)?,
                        gates_total: row.get(3)?,
                        changed_files: row.get(4)?,
                        known_gaps: row.get(5)?,
                        recovery_status: row.get(6)?,
                        generated_at: row.get(7)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn delete(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute("DELETE FROM proofs WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
