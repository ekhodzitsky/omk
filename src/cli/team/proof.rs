use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::runtime::config::EVENTS_FILE;
use crate::runtime::events::{EventBuilder, EventWriter, RunId};
use crate::runtime::proof::{Proof, ProofGenerator, ProofStatus};

#[derive(Debug, Serialize)]
struct FailureArtifact {
    run_id: String,
    status: String,
    readiness: String,
    proof_path: String,
    summary: String,
    failures: Vec<String>,
    known_gaps: Vec<String>,
}

pub(super) fn failure_artifact_path(state_dir: &Path) -> PathBuf {
    state_dir.join("failure.json")
}

pub(super) async fn finalize_team_run_proof(
    state_dir: &Path,
    event_writer: &EventWriter,
    run_id: &RunId,
) -> Result<Proof> {
    let event_log = state_dir.join(EVENTS_FILE);
    let proof = ProofGenerator::from_events(run_id, &event_log).await?;
    proof.save(state_dir).await?;

    let proof_path = Proof::proof_path(state_dir);
    if proof.status == ProofStatus::Ready {
        let failure_path = failure_artifact_path(state_dir);
        if failure_path.exists() {
            let _ = tokio::fs::remove_file(&failure_path).await;
        }
    } else {
        write_failure_artifact(state_dir, &proof, &proof_path).await?;
    }

    let event =
        EventBuilder::new(run_id.clone()).proof_written(&proof_path, &proof.status.to_string())?;
    event_writer.append(&event).await?;

    Ok(proof)
}

async fn write_failure_artifact(state_dir: &Path, proof: &Proof, proof_path: &Path) -> Result<()> {
    let artifact = FailureArtifact {
        run_id: proof.run_id.0.clone(),
        status: proof.status.to_string(),
        readiness: proof.readiness().to_string(),
        proof_path: proof_path.to_string_lossy().to_string(),
        summary: proof.summary.clone(),
        failures: proof
            .failures
            .iter()
            .map(|failure| failure.description.clone())
            .collect(),
        known_gaps: proof.known_gaps.clone(),
    };
    let json = serde_json::to_vec_pretty(&artifact)?;
    crate::runtime::atomic::atomic_write(&failure_artifact_path(state_dir), &json).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::events::{Event, EventKind, EventReader};

    #[tokio::test]
    async fn finalize_team_run_proof_writes_ready_proof_and_event() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("team").join("ready-run");
        tokio::fs::create_dir_all(&state_dir).await.unwrap();

        let event_log = state_dir.join(EVENTS_FILE);
        let event_writer = EventWriter::new(&event_log);
        let run_id = RunId("ready-run".to_string());
        let builder = EventBuilder::new(run_id.clone());

        event_writer
            .append(
                &builder
                    .run_started("team", temp.path(), "ship a ready proof")
                    .unwrap(),
            )
            .await
            .unwrap();
        event_writer
            .append(&builder.gate_passed_by_name("fmt").unwrap())
            .await
            .unwrap();
        event_writer.append(&builder.run_completed()).await.unwrap();

        let proof = finalize_team_run_proof(&state_dir, &event_writer, &run_id)
            .await
            .unwrap();

        assert_eq!(proof.status, ProofStatus::Ready);
        assert!(Proof::proof_path(&state_dir).exists());
        assert!(!failure_artifact_path(&state_dir).exists());

        let events = EventReader::read_all(&event_log).await.unwrap();
        assert!(events.iter().any(|event| {
            event.kind == EventKind::ProofWritten
                && event
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("status"))
                    .and_then(|status| status.as_str())
                    == Some("ready")
        }));
    }

    #[tokio::test]
    async fn finalize_team_run_proof_writes_failure_artifact_for_interrupt() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("team").join("interrupted-run");
        tokio::fs::create_dir_all(&state_dir).await.unwrap();

        let event_log = state_dir.join(EVENTS_FILE);
        let event_writer = EventWriter::new(&event_log);
        let run_id = RunId("interrupted-run".to_string());
        let builder = EventBuilder::new(run_id.clone());

        event_writer
            .append(
                &builder
                    .run_started("team", temp.path(), "interrupt a team run")
                    .unwrap(),
            )
            .await
            .unwrap();
        let interrupt =
            Event::new(run_id.clone(), EventKind::ManualInterrupt).with_actor("omk-cli");
        event_writer.append(&interrupt).await.unwrap();

        let proof = finalize_team_run_proof(&state_dir, &event_writer, &run_id)
            .await
            .unwrap();

        assert_eq!(proof.status, ProofStatus::Failed);
        assert!(Proof::proof_path(&state_dir).exists());

        let failure_json = tokio::fs::read_to_string(failure_artifact_path(&state_dir))
            .await
            .unwrap();
        let failure: serde_json::Value = serde_json::from_str(&failure_json).unwrap();
        assert_eq!(failure["status"], "failed");
        assert_eq!(failure["readiness"], "blocked");
        assert!(failure["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry == "manual interrupt"));

        let events = EventReader::read_all(&event_log).await.unwrap();
        assert!(events.iter().any(|event| {
            event.kind == EventKind::ProofWritten
                && event
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("status"))
                    .and_then(|status| status.as_str())
                    == Some("failed")
        }));
    }
}
