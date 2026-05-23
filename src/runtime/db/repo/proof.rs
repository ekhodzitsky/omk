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
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "INSERT INTO proofs (
                        goal_id, version, status, readiness, summary, task_graph_summary,
                        changed_files, commits, git, gates, gates_passed, gates_total,
                        post_mutation_gates_ran, known_gaps, human_decisions_required,
                        recovery_status, delivery_metadata, review_artifacts,
                        integration_evidence, oracle_evidence, generated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
                    ON CONFLICT(goal_id) DO UPDATE SET
                        version = excluded.version,
                        status = excluded.status,
                        readiness = excluded.readiness,
                        summary = excluded.summary,
                        task_graph_summary = excluded.task_graph_summary,
                        changed_files = excluded.changed_files,
                        commits = excluded.commits,
                        git = excluded.git,
                        gates = excluded.gates,
                        gates_passed = excluded.gates_passed,
                        gates_total = excluded.gates_total,
                        post_mutation_gates_ran = excluded.post_mutation_gates_ran,
                        known_gaps = excluded.known_gaps,
                        human_decisions_required = excluded.human_decisions_required,
                        recovery_status = excluded.recovery_status,
                        delivery_metadata = excluded.delivery_metadata,
                        review_artifacts = excluded.review_artifacts,
                        integration_evidence = excluded.integration_evidence,
                        oracle_evidence = excluded.oracle_evidence,
                        generated_at = excluded.generated_at",
                    params![
                        proof.goal_id,
                        proof.version,
                        proof.status,
                        proof.readiness,
                        proof.summary,
                        proof.task_graph_summary,
                        proof.changed_files,
                        proof.commits,
                        proof.git,
                        proof.gates,
                        proof.gates_passed,
                        proof.gates_total,
                        if proof.post_mutation_gates_ran { 1 } else { 0 },
                        proof.known_gaps,
                        proof.human_decisions_required,
                        proof.recovery_status,
                        proof.delivery_metadata,
                        proof.review_artifacts,
                        proof.integration_evidence,
                        proof.oracle_evidence,
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
            .call(
                move |conn| -> Result<Option<ProofRecord>, rusqlite::Error> {
                    let mut stmt = conn.prepare(
                        "SELECT
                        goal_id, version, status, readiness, summary, task_graph_summary,
                        changed_files, commits, git, gates, gates_passed, gates_total,
                        post_mutation_gates_ran, known_gaps, human_decisions_required,
                        recovery_status, delivery_metadata, review_artifacts,
                        integration_evidence, oracle_evidence, generated_at
                    FROM proofs WHERE goal_id = ?1",
                    )?;
                    let mut rows = stmt.query(params![goal_id])?;
                    if let Some(row) = rows.next()? {
                        Ok(Some(ProofRecord {
                            goal_id: row.get(0)?,
                            version: row.get(1)?,
                            status: row.get(2)?,
                            readiness: row.get(3)?,
                            summary: row.get(4)?,
                            task_graph_summary: row.get(5)?,
                            changed_files: row.get(6)?,
                            commits: row.get(7)?,
                            git: row.get(8)?,
                            gates: row.get(9)?,
                            gates_passed: row.get(10)?,
                            gates_total: row.get(11)?,
                            post_mutation_gates_ran: row.get::<_, i32>(12)? != 0,
                            known_gaps: row.get(13)?,
                            human_decisions_required: row.get(14)?,
                            recovery_status: row.get(15)?,
                            delivery_metadata: row.get(16)?,
                            review_artifacts: row.get(17)?,
                            integration_evidence: row.get(18)?,
                            oracle_evidence: row.get(19)?,
                            generated_at: row.get(20)?,
                        }))
                    } else {
                        Ok(None)
                    }
                },
            )
            .await
            .map_err(DbError::Connection)
    }

    async fn delete(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute("DELETE FROM proofs WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
