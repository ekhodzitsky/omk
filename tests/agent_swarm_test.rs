use anyhow::Result;
use chrono::Utc;
use omk::runtime::goal::{
    execute_goal_with_dispatcher, verify_goal_with_slices, GoalAgentRunEvidence, GoalDispatcher,
    GoalPhase, GoalState, GoalStatus, GoalTask, GoalTaskGraph, GoalTaskStatus,
};
use omk::runtime::scheduler::runner::RunSummary;
use omk::runtime::worker::{ResultStatus, WorkerResult};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// Lock to serialize tests that mutate process environment variables.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn setup_isolated_env() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let xdg_state = home.join(".local").join("state");
    std::fs::create_dir_all(&xdg_state).unwrap();
    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_STATE_HOME", &xdg_state);
    std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
    std::env::set_var("XDG_DATA_HOME", home.join(".local").join("share"));
    std::env::set_var("XDG_CACHE_HOME", home.join(".cache"));
    (tmp, xdg_state)
}

fn init_git_repo(dir: &Path) {
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(dir)
        .output()
        .expect("git init failed");
    assert!(output.status.success(), "git init failed");
}

async fn write_goal_state(
    state_dir: &Path,
    goal_id: &str,
    max_agents: usize,
    slice_execution: bool,
) {
    tokio::fs::create_dir_all(state_dir).await.unwrap();
    let state = GoalState {
        version: 1,
        goal_id: goal_id.to_string(),
        original_goal: "test swarm".to_string(),
        normalized_goal: "test swarm".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        until_ready: false,
        budget_time: None,
        budget_tokens: None,
        budget_usd: None,
        max_agents: Some(max_agents),
        cost_tracker_path: None,
        terminal_criteria: Default::default(),
        delivery_policy: omk::runtime::goal::GoalDeliveryPolicy::Local,
        merge_policy: omk::runtime::goal::GoalMergePolicy::Disabled,
        slice_execution,
        artifacts: vec![],
        failure: None,
        state_dir: state_dir.to_path_buf(),
        recovery_attempts: 0,
    };
    let json = serde_json::to_vec_pretty(&state).unwrap();
    tokio::fs::write(state_dir.join("goal.json"), json)
        .await
        .unwrap();
}

async fn write_task_graph_with_slices(state_dir: &Path, goal_id: &str, wt1: &Path, wt2: &Path) {
    tokio::fs::create_dir_all(state_dir).await.unwrap();
    let task_graph = serde_json::json!({
        "version": 1,
        "goal_id": goal_id,
        "generated_at": Utc::now(),
        "tasks": [
            {
                "id": "goal-local-verify",
                "title": "Local verification",
                "description": "Verify",
                "status": "pending",
                "dependencies": [],
                "read_set": [],
                "write_set": [],
                "risk": "low",
                "acceptance": ["Pass gates"]
            },
            {
                "id": "goal-agent-implement-1",
                "title": "Implement slice 1",
                "description": "Slice 1",
                "status": "pending",
                "dependencies": [],
                "read_set": ["src/a.rs"],
                "write_set": ["src/a.rs"],
                "risk": "low",
                "acceptance": ["Implement slice 1"],
                "delivery": {
                    "slice_id": "goal-agent-implement-1",
                    "worktree_path": wt1,
                    "status": "planned",
                    "dependencies": []
                }
            },
            {
                "id": "goal-agent-implement-2",
                "title": "Implement slice 2",
                "description": "Slice 2",
                "status": "pending",
                "dependencies": [],
                "read_set": ["src/b.rs"],
                "write_set": ["src/b.rs"],
                "risk": "low",
                "acceptance": ["Implement slice 2"],
                "delivery": {
                    "slice_id": "goal-agent-implement-2",
                    "worktree_path": wt2,
                    "status": "planned",
                    "dependencies": []
                }
            }
        ]
    });
    tokio::fs::write(
        state_dir.join("task-graph.json"),
        serde_json::to_vec_pretty(&task_graph).unwrap(),
    )
    .await
    .unwrap();
}

async fn write_passing_gates(project_dir: &Path) {
    let omk_dir = project_dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "pass"
command = "/bin/sh"
args = ["-c", "exit 0"]
required = false
"#,
    )
    .await
    .unwrap();
}

async fn write_empty_proof(state_dir: &Path, goal_id: &str) {
    tokio::fs::create_dir_all(state_dir).await.unwrap();
    let proof = serde_json::json!({
        "version": 1,
        "goal_id": goal_id,
        "status": "not_ready",
        "readiness": "incomplete",
        "summary": "",
        "generated_at": Utc::now(),
        "artifacts": [],
        "task_graph_summary": {
            "total_tasks": 3,
            "pending_tasks": 3,
            "blocked_tasks": 0,
            "done_tasks": 0
        },
        "changed_files": [],
        "commits": [],
        "gates": [],
        "post_mutation_gates_ran": false,
        "known_gaps": [],
        "human_decisions_required": []
    });
    tokio::fs::write(
        state_dir.join("proof.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .await
    .unwrap();
}

fn make_evidence(
    run_id: &str,
    completed: usize,
    failed: usize,
    task_id: &str,
) -> GoalAgentRunEvidence {
    GoalAgentRunEvidence {
        summary: RunSummary {
            run_id: run_id.to_string(),
            completed,
            failed,
            cancelled: 0,
            total: completed + failed,
        },
        run_path: PathBuf::from(format!("/tmp/run-{run_id}")),
        task_policy_path: PathBuf::new(),
        agent_task_proposals_path: PathBuf::new(),
        worker_outbox_path: PathBuf::new(),
        wire_events_path: PathBuf::new(),
        mutation_diff_path: PathBuf::new(),
        changed_files_path: PathBuf::new(),
        changed_files: vec![format!("src/{run_id}.rs")],
        accepted_task_count: 0,
        rejected_task_count: 0,
        accepted_task_ids: vec![task_id.to_string()],
        agent_proposed_tasks: vec![],
        worker_results: vec![WorkerResult {
            task_id: task_id.to_string(),
            status: if failed == 0 {
                ResultStatus::Success
            } else {
                ResultStatus::Failed
            },
            summary: "mock".to_string(),
            artifacts: vec![],
            elapsed_secs: 1,
        }],
        worker_summary: Some("mock summary".to_string()),
    }
}

#[derive(Clone)]
struct MockGoalDispatcher {
    results: std::sync::Arc<Mutex<Vec<Result<GoalAgentRunEvidence>>>>,
}

impl MockGoalDispatcher {
    fn new(results: Vec<Result<GoalAgentRunEvidence>>) -> Self {
        Self {
            results: std::sync::Arc::new(Mutex::new(results)),
        }
    }
}

impl GoalDispatcher for MockGoalDispatcher {
    async fn execute_wave(
        &self,
        _state: &GoalState,
        _task_graph: &GoalTaskGraph,
        _project_dir: &Path,
        _started_at: chrono::DateTime<chrono::Utc>,
        _dispatch: &omk::runtime::goal::GoalAgentDispatchPlan,
    ) -> Result<GoalAgentRunEvidence> {
        let mut results = self.results.lock().unwrap();
        results.remove(0)
    }

    async fn append_execution_events(
        &self,
        _state: &GoalState,
        _task: &GoalTask,
        _evidence: &GoalAgentRunEvidence,
    ) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn verify_goal_with_slices_runs_gates_in_parallel() {
    let _guard = ENV_LOCK.lock().await;
    let (_tmp, xdg_state) = setup_isolated_env();
    let goal_id = "goal-verify-slices";
    let state_dir = xdg_state.join("omk").join("goals").join(goal_id);
    let project_dir = state_dir.join("project");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    init_git_repo(&project_dir);
    write_passing_gates(&project_dir).await;
    write_goal_state(&state_dir, goal_id, 2, true).await;
    assert!(
        state_dir.join("goal.json").exists(),
        "goal-state.json should exist at {state_dir:?}"
    );
    let gs = tokio::fs::read_to_string(state_dir.join("goal.json"))
        .await
        .unwrap();
    eprintln!("DEBUG goal-state content: {gs}");

    let task_graph = serde_json::json!({
        "version": 1,
        "goal_id": goal_id,
        "generated_at": Utc::now(),
        "tasks": [
            {
                "id": "goal-local-verify",
                "title": "Local verification",
                "description": "Verify",
                "status": "pending",
                "dependencies": [],
                "read_set": [],
                "write_set": [],
                "risk": "low",
                "acceptance": ["Pass gates"]
            }
        ]
    });
    tokio::fs::write(
        state_dir.join("task-graph.json"),
        serde_json::to_vec_pretty(&task_graph).unwrap(),
    )
    .await
    .unwrap();

    let slices = vec![
        omk::runtime::goal::GoalDeliverySlice {
            slice_id: "slice-1".to_string(),
            task_id: "slice-1".to_string(),
            owner_role: "executor".to_string(),
            read_scope: vec![],
            write_scope: vec!["src/a.rs".to_string()],
            dependencies: vec![],
            branch_name: "slice-1".to_string(),
            worktree_name: "wt1".to_string(),
            worktree_path: project_dir.clone(),
            gates: vec![],
            review_needs: vec![],
            pr_url: None,
        },
        omk::runtime::goal::GoalDeliverySlice {
            slice_id: "slice-2".to_string(),
            task_id: "slice-2".to_string(),
            owner_role: "executor".to_string(),
            read_scope: vec![],
            write_scope: vec!["src/b.rs".to_string()],
            dependencies: vec![],
            branch_name: "slice-2".to_string(),
            worktree_name: "wt2".to_string(),
            worktree_path: project_dir.clone(),
            gates: vec![],
            review_needs: vec![],
            pr_url: None,
        },
    ];

    let proof = verify_goal_with_slices(goal_id, &project_dir, Some(&slices))
        .await
        .expect("verify_goal_with_slices should succeed");

    assert_eq!(proof.goal_id, goal_id);
    assert!(proof.gates.iter().any(|g| g.name == "pass" && g.passed));
}

#[tokio::test]
async fn two_concurrent_slices_both_succeed() {
    let _guard = ENV_LOCK.lock().await;
    let (_tmp, xdg_state) = setup_isolated_env();
    let goal_id = "goal-concurrent-success";
    let state_dir = xdg_state.join("omk").join("goals").join(goal_id);
    let project_dir = state_dir.join("project");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    init_git_repo(&project_dir);
    let wt1 = project_dir.join("wt1");
    let wt2 = project_dir.join("wt2");
    tokio::fs::create_dir_all(&wt1).await.unwrap();
    tokio::fs::create_dir_all(&wt2).await.unwrap();
    write_passing_gates(&project_dir).await;
    write_goal_state(&state_dir, goal_id, 2, true).await;
    write_task_graph_with_slices(&state_dir, goal_id, &wt1, &wt2).await;
    write_empty_proof(&state_dir, goal_id).await;

    let dispatcher = MockGoalDispatcher::new(vec![
        Ok(make_evidence("slice-1", 1, 0, "goal-agent-implement-1")),
        Ok(make_evidence("slice-2", 1, 0, "goal-agent-implement-2")),
    ]);

    let proof = execute_goal_with_dispatcher(goal_id, &project_dir, &dispatcher)
        .await
        .expect("execute_goal should succeed");

    assert_eq!(proof.goal_id, goal_id);

    let task_graph: GoalTaskGraph = serde_json::from_str(
        &tokio::fs::read_to_string(state_dir.join("task-graph.json"))
            .await
            .unwrap(),
    )
    .unwrap();

    for t in &task_graph.tasks {
        eprintln!("DEBUG task {} status={:?}", t.id, t.status);
    }

    let t1 = task_graph
        .tasks
        .iter()
        .find(|t| t.id == "goal-agent-implement-1")
        .expect("slice 1 task missing");
    let t2 = task_graph
        .tasks
        .iter()
        .find(|t| t.id == "goal-agent-implement-2")
        .expect("slice 2 task missing");
    assert_eq!(t1.status, GoalTaskStatus::Done);
    assert_eq!(t2.status, GoalTaskStatus::Done);
}

#[tokio::test]
async fn two_concurrent_slices_one_fails() {
    let _guard = ENV_LOCK.lock().await;
    let (_tmp, xdg_state) = setup_isolated_env();
    let goal_id = "goal-concurrent-partial";
    let state_dir = xdg_state.join("omk").join("goals").join(goal_id);
    let project_dir = state_dir.join("project");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    init_git_repo(&project_dir);
    let wt1 = project_dir.join("wt1");
    let wt2 = project_dir.join("wt2");
    tokio::fs::create_dir_all(&wt1).await.unwrap();
    tokio::fs::create_dir_all(&wt2).await.unwrap();
    write_passing_gates(&project_dir).await;
    write_goal_state(&state_dir, goal_id, 2, true).await;
    write_task_graph_with_slices(&state_dir, goal_id, &wt1, &wt2).await;
    write_empty_proof(&state_dir, goal_id).await;

    let dispatcher = MockGoalDispatcher::new(vec![
        Ok(make_evidence("slice-1", 1, 0, "goal-agent-implement-1")),
        Ok(make_evidence("slice-2", 0, 1, "goal-agent-implement-2")),
    ]);

    let proof = execute_goal_with_dispatcher(goal_id, &project_dir, &dispatcher)
        .await
        .expect("execute_goal should succeed even with partial failure");

    assert_eq!(proof.goal_id, goal_id);

    let task_graph: GoalTaskGraph = serde_json::from_str(
        &tokio::fs::read_to_string(state_dir.join("task-graph.json"))
            .await
            .unwrap(),
    )
    .unwrap();

    let t1 = task_graph
        .tasks
        .iter()
        .find(|t| t.id == "goal-agent-implement-1")
        .expect("slice 1 task missing");
    let t2 = task_graph
        .tasks
        .iter()
        .find(|t| t.id == "goal-agent-implement-2")
        .expect("slice 2 task missing");
    assert_eq!(t1.status, GoalTaskStatus::Done);
    assert_eq!(t2.status, GoalTaskStatus::Blocked);
}

#[tokio::test]
async fn cost_tracking_reports_actual_worker_count() {
    let _guard = ENV_LOCK.lock().await;
    let (_tmp, xdg_state) = setup_isolated_env();
    let goal_id = "goal-cost-tracking";
    let state_dir = xdg_state.join("omk").join("goals").join(goal_id);
    let project_dir = state_dir.join("project");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    init_git_repo(&project_dir);
    let wt1 = project_dir.join("wt1");
    let wt2 = project_dir.join("wt2");
    tokio::fs::create_dir_all(&wt1).await.unwrap();
    tokio::fs::create_dir_all(&wt2).await.unwrap();
    write_passing_gates(&project_dir).await;
    write_goal_state(&state_dir, goal_id, 2, true).await;
    write_task_graph_with_slices(&state_dir, goal_id, &wt1, &wt2).await;
    write_empty_proof(&state_dir, goal_id).await;

    let dispatcher = MockGoalDispatcher::new(vec![
        Ok(make_evidence("slice-1", 1, 0, "goal-agent-implement-1")),
        Ok(make_evidence("slice-2", 1, 0, "goal-agent-implement-2")),
    ]);

    execute_goal_with_dispatcher(goal_id, &project_dir, &dispatcher)
        .await
        .expect("execute_goal should succeed");

    let costs_path = state_dir.join("cost.jsonl");
    let costs: Vec<omk::cost::types::SessionCost> = if costs_path.exists() {
        let content = tokio::fs::read_to_string(&costs_path).await.unwrap();
        serde_json::from_str(&content).expect("valid cost JSON")
    } else {
        Vec::new()
    };
    assert!(
        !costs.is_empty(),
        "cost tracking should have recorded a session cost"
    );

    let last = costs.last().unwrap();
    assert_eq!(
        last.estimate.worker_count, 2,
        "cost tracking should report 2 concurrent workers"
    );
}
