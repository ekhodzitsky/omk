use anyhow::{Context, Result};
use chrono::Utc;

use crate::runtime::goal::evidence::record_artifact_path_once;
use crate::runtime::goal::open_pr::render_goal_open_pr;
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{
    FileSystemGoalStateStore, GoalState, GoalStateStore, GOAL_PROOF_FILE,
};
use crate::runtime::goal::{
    delivery::{GoalGithubPrClient, GoalMergePolicy},
    resolve_goal,
};

/// Merge the GitHub PR for a ready goal using the provided PR client.
///
/// Preconditions:
/// - `merge_policy` permits merge (`Gated` or `Manual`).
/// - `proof.status` is `Ready` and all gates pass.
/// - A PR URL exists in the goal's delivery metadata.
///
/// Postconditions:
/// - The PR is merged via `client.merge_pr(pr_url)`.
/// - The goal state is updated with a `pr_merge` artifact.
pub async fn merge_goal(goal_id: &str, client: &mut impl GoalGithubPrClient) -> Result<GoalState> {
    let mut state = resolve_goal(goal_id).await?;

    if !state.merge_policy.permits_merge() && state.merge_policy != GoalMergePolicy::Manual {
        anyhow::bail!(
            "Goal '{}' has merge policy '{}' which does not permit merge. \
             Only 'gated' or 'manual' policies allow merge.",
            state.goal_id,
            state.merge_policy.as_str()
        );
    }

    let proof = GoalProof::load(&state.state_dir)
        .await
        .with_context(|| format!("Failed to load goal proof for {}", state.goal_id))?;

    proof
        .validate_for_merge()
        .with_context(|| format!("Goal '{}' is not eligible for merge", state.goal_id))?;

    let draft = render_goal_open_pr(goal_id, false, false)
        .await
        .with_context(|| format!("Failed to render PR draft for {}", state.goal_id))?;

    let pr_url = draft.existing_pr_url.with_context(|| {
        format!(
            "Goal '{}' has no associated PR URL. \
             Ensure the goal was delivered with a PR before merging.",
            state.goal_id
        )
    })?;

    client.validate_merge_gate(&pr_url).await.with_context(|| {
        format!(
            "Merge gate validation failed for PR {pr_url} on goal {}",
            state.goal_id
        )
    })?;

    client
        .merge_pr(&pr_url)
        .await
        .with_context(|| format!("Failed to merge PR {pr_url} for goal {}", state.goal_id))?;

    let now = Utc::now();
    record_artifact_path_once(
        &mut state,
        "pr_merge",
        std::path::PathBuf::from(&pr_url),
        now,
    );
    record_artifact_path_once(
        &mut state,
        "delivery_evidence",
        std::path::PathBuf::from(&pr_url),
        now,
    );
    state.updated_at = now;
    FileSystemGoalStateStore::new()
        .save(&state)
        .await
        .with_context(|| format!("Failed to save goal state for {}", state.goal_id))?;

    // Update proof with merge evidence and ready status
    let mut proof = GoalProof::load(&state.state_dir)
        .await
        .with_context(|| format!("Failed to reload goal proof for {}", state.goal_id))?;
    proof.status = crate::runtime::goal::state::GoalStatus::Ready;
    proof.readiness = "ready: PR merged after passing merge gate".to_string();
    proof.artifacts = state.artifacts.clone();
    crate::runtime::goal::proof::write_json_artifact(
        &state.state_dir.join(GOAL_PROOF_FILE),
        &proof,
    )
    .await
    .with_context(|| format!("Failed to write merged proof for {}", state.goal_id))?;

    Ok(state)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;

    use super::*;
    use crate::runtime::goal::delivery::{
        GoalGithubPrMutation, GoalGithubPrOperation, GoalGithubPrRequest,
    };
    use crate::runtime::goal::state::{CreateGoalOptions, GoalPhase, GoalStatus, GOAL_PROOF_FILE};
    use crate::runtime::goal::{create_goal, FileSystemGoalStateStore, GoalProof, GoalStateStore};

    #[derive(Debug, Default)]
    struct MockMergeClient {
        merge_calls: Vec<String>,
        fail_next: Option<String>,
        gate_fail_next: Option<String>,
    }

    impl GoalGithubPrClient for MockMergeClient {
        fn create_pr<'a>(
            &'a mut self,
            _request: GoalGithubPrRequest,
        ) -> Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>> {
            unimplemented!()
        }

        fn update_pr<'a>(
            &'a mut self,
            _request: GoalGithubPrRequest,
        ) -> Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>> {
            unimplemented!()
        }

        fn merge_pr<'a>(
            &'a mut self,
            pr_url: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>> {
            self.merge_calls.push(pr_url.to_string());
            if let Some(ref err) = self.fail_next {
                let err = err.clone();
                return Box::pin(async move { anyhow::bail!("{err}") });
            }
            Box::pin(async move {
                Ok(GoalGithubPrMutation {
                    operation: GoalGithubPrOperation::Create,
                    url: Some(pr_url.to_string()),
                })
            })
        }

        fn validate_merge_gate<'a>(
            &'a mut self,
            _pr_url: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
            if let Some(ref err) = self.gate_fail_next {
                let err = err.clone();
                return Box::pin(async move { anyhow::bail!("{err}") });
            }
            Box::pin(async move { Ok(()) })
        }
    }

    fn save_env() -> Vec<(&'static str, Option<std::ffi::OsString>)> {
        vec![
            ("XDG_STATE_HOME", std::env::var_os("XDG_STATE_HOME")),
            ("XDG_CONFIG_HOME", std::env::var_os("XDG_CONFIG_HOME")),
            ("XDG_DATA_HOME", std::env::var_os("XDG_DATA_HOME")),
            ("XDG_CACHE_HOME", std::env::var_os("XDG_CACHE_HOME")),
            ("HOME", std::env::var_os("HOME")),
        ]
    }

    fn restore_env(saved: Vec<(&'static str, Option<std::ffi::OsString>)>) {
        for (key, value) in saved {
            if let Some(v) = value {
                std::env::set_var(key, v);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    async fn create_ready_goal_with_pr() -> (tempfile::TempDir, String, String) {
        let (tmp, envs) = crate::test_helpers::isolated_xdg_env();
        for (key, value) in &envs {
            std::env::set_var(key, value);
        }
        let xdg_state = envs
            .iter()
            .find(|(k, _)| *k == "XDG_STATE_HOME")
            .map(|(_, v)| v)
            .unwrap();
        std::fs::create_dir_all(xdg_state.join("omk")).unwrap();

        let now = chrono::Utc::now();
        let goal_id = "goal-merge-test-01".to_string();
        let state_dir = crate::runtime::goal::state::goals_dir().join(&goal_id);
        crate::runtime::config::ensure_private_dir(&state_dir)
            .await
            .expect("create goal dir");

        let state = crate::runtime::goal::state::GoalState {
            version: 1,
            goal_id: goal_id.clone(),
            original_goal: "Implement a testable feature with acceptance criteria".to_string(),
            normalized_goal: "implement a testable feature with acceptance criteria".to_string(),
            status: GoalStatus::Ready,
            phase: GoalPhase::Proof,
            created_at: now,
            updated_at: now,
            completed_at: Some(now),
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            terminal_criteria: crate::runtime::goal::state::GoalTerminalCriteria::default(),
            artifacts: Vec::new(),
            failure: None,
            state_dir: state_dir.clone(),
            cost_tracker_path: None,
            delivery_policy: crate::runtime::goal::GoalDeliveryPolicy::Local,
            merge_policy: GoalMergePolicy::Gated,
            slice_execution: false,
            recovery_attempts: 0,
        };

        let proof = GoalProof {
            version: 1,
            goal_id: goal_id.clone(),
            status: GoalStatus::Ready,
            readiness: "ready".to_string(),
            summary: "ready".to_string(),
            generated_at: now,
            artifacts: Vec::new(),
            task_graph_summary: crate::runtime::goal::task_graph::GoalTaskGraphSummary {
                total_tasks: 1,
                pending_tasks: 0,
                blocked_tasks: 0,
                done_tasks: 1,
            },
            changed_files: vec!["src/main.rs".to_string()],
            commits: Vec::new(),
            git: None,
            gates: vec![crate::runtime::gates::GateResult {
                name: "test".to_string(),
                passed: true,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 0,
                required: true,
                command_line: String::new(),
                exit_code: Some(0),
                timed_out: false,
                stdout_summary: None,
                stderr_summary: None,
                output_path: None,
                timeout_secs: 0,
                circuit_breaker_open: false,
            }],
            post_mutation_gates_ran: true,
            known_gaps: Vec::new(),
            human_decisions_required: Vec::new(),
            recovery_status: None,
        };

        let task_graph = crate::runtime::goal::task_graph::GoalTaskGraph {
            version: 1,
            goal_id: goal_id.clone(),
            generated_at: now,
            tasks: vec![crate::runtime::goal::task_graph::GoalTask {
                id: "integrator".to_string(),
                title: "Integrator delivery".to_string(),
                description: "Deliver the integrator PR".to_string(),
                status: crate::runtime::goal::task_graph::GoalTaskStatus::Done,
                owner_role: None,
                completed_at: Some(now),
                evidence: Vec::new(),
                retry_count: 0,
                max_retries: 0,
                lease_expires_at: None,
                dependencies: Vec::new(),
                read_set: Vec::new(),
                write_set: Vec::new(),
                risk: "low".to_string(),
                acceptance: vec!["PR delivered".to_string()],
            }],
        };

        task_graph.save(&state_dir).await.expect("save task graph");

        let update = crate::runtime::goal::task_graph::GoalTaskDeliveryMetadataUpdate {
            pr_url: Some("https://github.com/example/omk/pull/42".to_string()),
            ..Default::default()
        };
        crate::runtime::goal::task_graph::update_goal_task_delivery_metadata(
            &state_dir,
            "integrator",
            update,
        )
        .await
        .expect("update delivery metadata");

        crate::runtime::goal::proof::write_json_artifact(&state_dir.join(GOAL_PROOF_FILE), &proof)
            .await
            .expect("write proof");
        FileSystemGoalStateStore::new()
            .save(&state)
            .await
            .expect("save state");

        (tmp, goal_id, state_dir.to_string_lossy().to_string())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_succeeds_for_ready_goal_with_pr() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, _state_dir) = create_ready_goal_with_pr().await;
        let mut client = MockMergeClient::default();

        let state = merge_goal(&goal_id, &mut client).await.expect("merge_goal");

        assert_eq!(client.merge_calls.len(), 1);
        assert_eq!(
            client.merge_calls[0],
            "https://github.com/example/omk/pull/42"
        );
        assert_eq!(state.status, GoalStatus::Ready);
        assert!(state.artifacts.iter().any(|a| a.kind == "pr_merge"));
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_fails_when_proof_not_ready() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (tmp, envs) = crate::test_helpers::isolated_xdg_env();
        for (key, value) in &envs {
            std::env::set_var(key, value);
        }
        let xdg_state = envs
            .iter()
            .find(|(k, _)| *k == "XDG_STATE_HOME")
            .map(|(_, v)| v)
            .unwrap();
        std::fs::create_dir_all(xdg_state.join("omk")).unwrap();

        let goal = create_goal(
            "Implement a testable feature",
            CreateGoalOptions {
                until_ready: false,
                budget_time: None,
                budget_tokens: None,
                budget_usd: None,
                max_agents: None,
                delivery_policy: crate::runtime::goal::GoalDeliveryPolicy::Local,
                merge_policy: GoalMergePolicy::Gated,
                slice_execution: false,
                enforce_protection: false,
            },
            None,
        )
        .await
        .expect("create goal");

        let mut client = MockMergeClient::default();
        let err = merge_goal(&goal.goal_id, &mut client)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("not eligible for merge"),
            "expected not-eligible error, got: {err}"
        );
        assert!(client.merge_calls.is_empty());

        drop(tmp);
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_fails_when_policy_disabled() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (tmp, envs) = crate::test_helpers::isolated_xdg_env();
        for (key, value) in &envs {
            std::env::set_var(key, value);
        }

        let goal = create_goal(
            "Implement a testable feature",
            CreateGoalOptions {
                until_ready: false,
                budget_time: None,
                budget_tokens: None,
                budget_usd: None,
                max_agents: None,
                delivery_policy: crate::runtime::goal::GoalDeliveryPolicy::Local,
                merge_policy: GoalMergePolicy::Disabled,
                slice_execution: false,
                enforce_protection: false,
            },
            None,
        )
        .await
        .expect("create goal");

        let mut client = MockMergeClient::default();
        let err = merge_goal(&goal.goal_id, &mut client)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("does not permit merge"),
            "expected policy error, got: {err}"
        );
        assert!(client.merge_calls.is_empty());

        drop(tmp);
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_propagates_client_error() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, _state_dir) = create_ready_goal_with_pr().await;
        let mut client = MockMergeClient {
            fail_next: Some("simulated merge failure".to_string()),
            ..Default::default()
        };

        let err = merge_goal(&goal_id, &mut client).await.unwrap_err();
        let err_str = format!("{err:?}");
        assert!(
            err_str.contains("simulated merge failure"),
            "expected client error, got: {err_str}"
        );
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_fails_when_gate_ci_fails() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, _state_dir) = create_ready_goal_with_pr().await;
        let mut client = MockMergeClient {
            gate_fail_next: Some("CI check 'test' failed".to_string()),
            ..Default::default()
        };

        let err = merge_goal(&goal_id, &mut client).await.unwrap_err();
        let err_str = format!("{err:?}");
        assert!(
            err_str.contains("Merge gate validation failed"),
            "expected gate error, got: {err_str}"
        );
        assert!(err_str.contains("CI check 'test' failed"), "got: {err_str}");
        assert!(client.merge_calls.is_empty());
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_fails_when_gate_merge_conflict() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, _state_dir) = create_ready_goal_with_pr().await;
        let mut client = MockMergeClient {
            gate_fail_next: Some("PR has merge conflicts".to_string()),
            ..Default::default()
        };

        let err = merge_goal(&goal_id, &mut client).await.unwrap_err();
        let err_str = format!("{err:?}");
        assert!(err_str.contains("Merge gate validation failed"));
        assert!(err_str.contains("merge conflicts"));
        assert!(client.merge_calls.is_empty());
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_fails_when_gate_review_blocked() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, _state_dir) = create_ready_goal_with_pr().await;
        let mut client = MockMergeClient {
            gate_fail_next: Some("PR requires review approval".to_string()),
            ..Default::default()
        };

        let err = merge_goal(&goal_id, &mut client).await.unwrap_err();
        let err_str = format!("{err:?}");
        assert!(err_str.contains("Merge gate validation failed"));
        assert!(err_str.contains("review approval"));
        assert!(client.merge_calls.is_empty());
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_fails_when_gate_branch_protection_missing() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, _state_dir) = create_ready_goal_with_pr().await;
        let mut client = MockMergeClient {
            gate_fail_next: Some(
                "branch protection not configured for base branch 'main'".to_string(),
            ),
            ..Default::default()
        };

        let err = merge_goal(&goal_id, &mut client).await.unwrap_err();
        let err_str = format!("{err:?}");
        assert!(err_str.contains("Merge gate validation failed"));
        assert!(err_str.contains("branch protection not configured"));
        assert!(client.merge_calls.is_empty());
        restore_env(saved);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn merge_goal_succeeds_for_manual_policy() {
        let _guard = crate::test_helpers::TEST_MUTEX.lock().await;
        let saved = save_env();
        let (_tmp, goal_id, state_dir) = create_ready_goal_with_pr().await;

        // Flip policy to Manual
        let mut state = FileSystemGoalStateStore::new()
            .load(std::path::Path::new(&state_dir))
            .await
            .expect("load state");
        state.merge_policy = GoalMergePolicy::Manual;
        FileSystemGoalStateStore::new()
            .save(&state)
            .await
            .expect("save manual state");

        let mut client = MockMergeClient::default();
        let state = merge_goal(&goal_id, &mut client).await.expect("merge_goal");

        assert_eq!(client.merge_calls.len(), 1);
        assert_eq!(
            client.merge_calls[0],
            "https://github.com/example/omk/pull/42"
        );
        assert_eq!(state.status, GoalStatus::Ready);
        assert!(state.artifacts.iter().any(|a| a.kind == "pr_merge"));
        assert!(state
            .artifacts
            .iter()
            .any(|a| a.kind == "delivery_evidence"));

        // Verify proof was updated to Ready
        let proof = GoalProof::load(std::path::Path::new(&state_dir))
            .await
            .expect("load proof");
        assert_eq!(proof.status, GoalStatus::Ready);
        assert!(proof
            .readiness
            .contains("PR merged after passing merge gate"));
        restore_env(saved);
    }
}
