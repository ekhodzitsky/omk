use anyhow::Result;
use chrono::Utc;

use crate::runtime::goal::{
    evidence,
    proof::{self, GoalProof},
    state::{self, GoalState, GoalStateStore},
    task_graph::GoalTaskGraph,
};

/// List all goals from the default filesystem store, newest first.
pub async fn list_goals() -> Result<Vec<GoalState>> {
    list_goals_with_store(&state::FileSystemGoalStateStore::new()).await
}

/// Generic variant of [`list_goals`] for tests and alternative backends.
pub async fn list_goals_with_store<S: GoalStateStore>(store: &S) -> Result<Vec<GoalState>> {
    store.list().await
}

/// Resolve a goal by id (or `"latest"`) using the default filesystem store.
pub async fn resolve_goal(goal_id: &str) -> Result<GoalState> {
    resolve_goal_with_store(&state::FileSystemGoalStateStore::new(), goal_id).await
}

/// Generic variant of [`resolve_goal`] for tests and alternative backends.
pub async fn resolve_goal_with_store<S: GoalStateStore>(
    store: &S,
    goal_id: &str,
) -> Result<GoalState> {
    if goal_id == "latest" {
        let mut goals = list_goals_with_store(store).await?;
        if let Some(goal) = goals.drain(..).next() {
            return Ok(goal);
        }
        anyhow::bail!(
            "No goals found in {}\n\nCreate one with:\n  omk goal run \"<engineering goal>\"",
            state::goals_dir().display()
        );
    }

    let goal_dir = state::goals_dir().join(goal_id);
    store.load(&goal_dir).await.map_err(|e| {
        if e.downcast_ref::<state::GoalStateError>()
            .is_some_and(|ge| matches!(ge, state::GoalStateError::MissingFile { .. }))
        {
            anyhow::anyhow!(
                "Goal '{goal_id}' not found.\n\nList existing goals:\n  omk goal list\n\nState directory: {}",
                state::goals_dir().display()
            )
        } else {
            e
        }
    })
}

/// Resolve proof for a goal, with recovery fallback.
pub async fn resolve_goal_proof(goal_id: &str) -> Result<GoalProof> {
    let goal = resolve_goal(goal_id).await?;
    resolve_proof_for_goal(&goal).await
}

/// Resolve proof for a goal that has already been loaded.
pub async fn resolve_proof_for_goal(goal: &GoalState) -> Result<GoalProof> {
    match GoalProof::load(&goal.state_dir).await {
        Ok(mut proof) => {
            proof::reconcile_goal_proof_with_state(&mut proof, goal);
            Ok(proof)
        }
        Err(error) => {
            let (task_graph, task_graph_gap) = match GoalTaskGraph::load(&goal.state_dir).await {
                Ok(graph) => (graph, None),
                Err(graph_error) => {
                    let gap = format!(
                        "Task graph could not be loaded while rebuilding proof: {}",
                        graph_error.root_cause()
                    );
                    (
                        GoalTaskGraph {
                            version: 1,
                            goal_id: goal.goal_id.clone(),
                            generated_at: Utc::now(),
                            tasks: Vec::new(),
                        },
                        Some(gap),
                    )
                }
            };
            let git = evidence::detect_git_evidence(&goal.state_dir).await;
            let mut proof = proof::build_scaffold_proof(goal, &task_graph, git, Utc::now());
            proof.known_gaps.push(format!(
                "Proof file could not be loaded; rebuilt from state: {}",
                error.root_cause()
            ));
            if let Some(gap) = task_graph_gap {
                proof.known_gaps.push(gap);
            }
            proof.recovery_status = Some(format!(
                "recovered: proof rebuilt from state because load failed: {}",
                error.root_cause()
            ));
            Ok(proof)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::goal::state::{GoalPhase, GoalStatus, InMemoryGoalStateStore};
    use chrono::Utc;

    fn test_state(goal_id: &str, created_at: chrono::DateTime<Utc>) -> GoalState {
        GoalState {
            version: 1,
            goal_id: goal_id.to_string(),
            original_goal: format!("Goal {goal_id}"),
            normalized_goal: format!("Goal {goal_id}"),
            status: GoalStatus::Running,
            phase: GoalPhase::Execution,
            created_at,
            updated_at: created_at,
            completed_at: None,
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            terminal_criteria: Default::default(),
            artifacts: vec![],
            failure: None,
            state_dir: std::path::PathBuf::from(format!("/tmp/omk/goals/{goal_id}")),
        }
    }

    #[tokio::test]
    async fn list_goals_returns_sorted_newest_first() {
        let store = InMemoryGoalStateStore::new();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(10);

        let older = test_state("goal-a", t1);
        let newer = test_state("goal-b", t2);

        store.save(&older).await.unwrap();
        store.save(&newer).await.unwrap();

        let goals = list_goals_with_store(&store).await.unwrap();
        assert_eq!(goals.len(), 2);
        assert_eq!(goals[0].goal_id, "goal-b");
        assert_eq!(goals[1].goal_id, "goal-a");
    }

    #[tokio::test]
    async fn list_goals_returns_empty_when_no_goals() {
        let store = InMemoryGoalStateStore::new();
        let goals = list_goals_with_store(&store).await.unwrap();
        assert!(goals.is_empty());
    }

    #[tokio::test]
    async fn resolve_goal_loads_existing_goal() {
        let store = InMemoryGoalStateStore::new();
        let mut state = test_state("goal-x", Utc::now());
        state.state_dir = state::goals_dir().join("goal-x");
        let dir = state.state_dir.clone();
        store.save(&state).await.unwrap();

        let resolved = resolve_goal_with_store(&store, "goal-x").await.unwrap();
        assert_eq!(resolved.goal_id, "goal-x");
        assert_eq!(resolved.state_dir, dir);
    }

    #[tokio::test]
    async fn resolve_goal_latest_returns_newest() {
        let store = InMemoryGoalStateStore::new();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(10);

        store.save(&test_state("goal-a", t1)).await.unwrap();
        store.save(&test_state("goal-b", t2)).await.unwrap();

        let resolved = resolve_goal_with_store(&store, "latest").await.unwrap();
        assert_eq!(resolved.goal_id, "goal-b");
    }

    #[tokio::test]
    async fn resolve_goal_missing_returns_error() {
        let store = InMemoryGoalStateStore::new();
        let result = resolve_goal_with_store(&store, "nonexistent").await;
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("nonexistent"), "error should mention the goal id");
    }
}
