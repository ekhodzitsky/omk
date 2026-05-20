use anyhow::Result;
use std::path::Path;

use crate::git::GitRepo;
use crate::runtime::goal::state;
use crate::runtime::goal::state::GoalStatus;
use crate::runtime::goal::types::{GoalControllerStep, GoalControllerStepKind};

pub(super) async fn run_integrator_gates(
    steps: &mut Vec<GoalControllerStep>,
    project_dir: &Path,
    state: &state::GoalState,
    base_branch: &str,
) -> Result<usize> {
    let integrator_gate_config = crate::runtime::gates::load_or_detect_gates(project_dir).await;
    let integrator_gate_artifacts = state
        .state_dir
        .join(state::GOAL_ARTIFACTS_DIR)
        .join(state::GOAL_GATE_ARTIFACTS_DIR)
        .join("integrator");
    let integrator_gates = crate::runtime::gates::run_gates_with_evidence(
        &integrator_gate_config,
        project_dir,
        Some(&integrator_gate_artifacts),
    )
    .await;
    let _ = crate::runtime::goal::verifier::append_gate_events(state, &integrator_gates).await;
    let integrator_gates_ok =
        !integrator_gates.is_empty() && crate::runtime::gates::gates_passed(&integrator_gates);
    if !integrator_gates_ok {
        if let Ok(repo) = GitRepo::open(project_dir) {
            let _ = repo.checkout(base_branch).await;
        }
        anyhow::bail!("integrator verification gates failed; switched back to base branch");
    }
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Verify,
        status: GoalStatus::Ready,
        summary: format!(
            "integrator verification wall passed ({} gate(s))",
            integrator_gates.len()
        ),
    });
    Ok(integrator_gates.len())
}
