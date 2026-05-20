use super::{
    format_goal_duration_secs, is_safe_goal_agent_path, normalize_goal, DbGoalStateStore,
    FileSystemGoalStateStore, GoalPhase, GoalState, GoalStateStore, GoalStatus, GOAL_STATE_FILE,
};
use std::fs;

#[test]
fn goal_status_serializes_as_snake_case() {
    let value = serde_json::to_value(GoalStatus::NotReady).unwrap();
    assert_eq!(value, "not_ready");
}

#[test]
fn paused_goal_status_serializes_as_snake_case() {
    let value = serde_json::to_value(GoalStatus::Paused).unwrap();
    assert_eq!(value, "paused");
}

#[test]
fn goal_phase_serializes_as_snake_case() {
    let value = serde_json::to_value(GoalPhase::VerificationDesign).unwrap();
    assert_eq!(value, "verification_design");
}

#[test]
fn normalize_goal_collapses_whitespace() {
    assert_eq!(normalize_goal("  ship   it\nwell  "), "ship it well");
}

#[test]
fn goal_duration_formats_to_stable_compact_units() {
    assert_eq!(format_goal_duration_secs(0), "0s");
    assert_eq!(format_goal_duration_secs(59), "59s");
    assert_eq!(format_goal_duration_secs(60), "1m");
    assert_eq!(format_goal_duration_secs(3_600), "1h");
    assert_eq!(format_goal_duration_secs(86_400), "1d");
}

#[tokio::test]
async fn goal_state_loads_legacy_json_with_safe_defaults() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(GOAL_STATE_FILE),
        r#"{
              "goal_id": "goal-legacy",
              "original_goal": "Ship safely",
              "normalized_goal": "Ship safely",
              "status": "not_ready",
              "created_at": "2026-05-13T00:00:00Z",
              "updated_at": "2026-05-13T00:00:01Z"
            }"#,
    )
    .unwrap();

    let state = FileSystemGoalStateStore::new()
        .load(temp.path())
        .await
        .unwrap();

    assert_eq!(state.version, 1);
    assert_eq!(state.phase, GoalPhase::Intake);
    assert!(!state.until_ready);
    assert!(state.terminal_criteria.proof_required);
    assert!(state.terminal_criteria.gates_required);
    assert!(state.terminal_criteria.human_blockers_stop);
    assert!(state.artifacts.is_empty());
    assert_eq!(state.state_dir, temp.path());
}

#[tokio::test]
async fn goal_state_load_rehomes_stale_persisted_state_dir() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(GOAL_STATE_FILE),
        r#"{
              "version": 1,
              "goal_id": "goal-moved",
              "original_goal": "Resume after move",
              "normalized_goal": "Resume after move",
              "status": "paused",
              "phase": "proof",
              "created_at": "2026-05-13T00:00:00Z",
              "updated_at": "2026-05-13T00:00:01Z",
              "until_ready": true,
              "terminal_criteria": {
                "proof_required": true,
                "gates_required": true,
                "human_blockers_stop": true
              },
              "state_dir": "/old/machine/.local/state/omk/goals/goal-moved"
            }"#,
    )
    .unwrap();

    let state = FileSystemGoalStateStore::new()
        .load(temp.path())
        .await
        .unwrap();

    assert_eq!(state.goal_id, "goal-moved");
    assert_eq!(state.status, GoalStatus::Paused);
    assert_eq!(state.state_dir, temp.path());
}

#[test]
fn is_safe_goal_agent_path_accepts_typical_repo_paths() {
    for ok in [
        "project files",
        "README.md",
        "src/lib.rs",
        "tests/fixtures/data.json",
        "./Cargo.toml",
        "docs/architecture.md",
    ] {
        assert!(
            is_safe_goal_agent_path(ok),
            "expected '{ok}' to be accepted"
        );
    }
}

#[test]
fn is_safe_goal_agent_path_rejects_empty_and_absolute() {
    assert!(!is_safe_goal_agent_path(""));
    assert!(!is_safe_goal_agent_path("   "));
    assert!(!is_safe_goal_agent_path("/etc/passwd"));
    assert!(!is_safe_goal_agent_path("/Users/me/repo/src/lib.rs"));
}

#[test]
fn is_safe_goal_agent_path_rejects_traversal_variants() {
    assert!(!is_safe_goal_agent_path(".."));
    assert!(!is_safe_goal_agent_path("../etc"));
    assert!(!is_safe_goal_agent_path("../../escape"));
    assert!(!is_safe_goal_agent_path("foo/../bar"));
    assert!(!is_safe_goal_agent_path("src/../../escape"));
}

#[test]
fn is_safe_goal_agent_path_rejects_home_expansion_attempts() {
    // A literal tilde at the start is a strong intent signal that the
    // agent expects HOME expansion. Refuse it even though Rust file ops
    // would not expand it on their own.
    assert!(!is_safe_goal_agent_path("~/.bashrc"));
    assert!(!is_safe_goal_agent_path("~root/.ssh/authorized_keys"));
    assert!(!is_safe_goal_agent_path("~"));
}

#[test]
fn is_safe_goal_agent_path_rejects_control_characters() {
    assert!(!is_safe_goal_agent_path("README\n.md"));
    assert!(!is_safe_goal_agent_path("a\0b"));
    assert!(!is_safe_goal_agent_path("path\twith\ttabs"));
    assert!(!is_safe_goal_agent_path("foo\x07bar"));
}

#[test]
fn is_safe_goal_agent_path_rejects_dot_git_family_at_any_depth() {
    // First-component rejection: the historical .git/.git/ check is
    // preserved and extended to every dotfile a tool might consume.
    assert!(!is_safe_goal_agent_path(".git"));
    assert!(!is_safe_goal_agent_path(".git/config"));
    assert!(!is_safe_goal_agent_path(".gitignore"));
    assert!(!is_safe_goal_agent_path(".gitmodules"));
    assert!(!is_safe_goal_agent_path(".gitattributes"));
    // GitHub Actions metadata is a code-execution surface for CI.
    assert!(!is_safe_goal_agent_path(".github"));
    assert!(!is_safe_goal_agent_path(".github/workflows/ci.yml"));
    // GitLab CI metadata is the analogous surface for GitLab pipelines.
    assert!(!is_safe_goal_agent_path(".gitlab-ci.yml"));
    // Sub-component traversal: a path that smuggles `.git` deeper in the
    // tree (e.g., a submodule clone target) must still be rejected.
    assert!(!is_safe_goal_agent_path("vendor/.git/HEAD"));
    assert!(!is_safe_goal_agent_path("apps/web/.github/workflows/x.yml"));
}

#[test]
fn is_safe_goal_agent_path_preserves_special_alias() {
    // The `project files` alias is preserved across whitespace trimming
    // and stays exempt from per-path validation. Mixed-case variants are
    // not the alias and instead fall through to the regular path policy
    // (which accepts them as a literal relative file name).
    assert!(is_safe_goal_agent_path("project files"));
    assert!(is_safe_goal_agent_path("  project files  "));
}

#[tokio::test]
async fn db_goal_state_store_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("goals.db");
    let db = crate::runtime::db::DbHandle::open(&db_path).await.unwrap();
    let store = DbGoalStateStore::new(db);

    let goal_dir = dir.path().join("goal-test-1");
    let state = GoalState {
        version: 1,
        goal_id: "goal-test-1".to_string(),
        original_goal: "Test goal".to_string(),
        normalized_goal: "test goal".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        completed_at: None,
        until_ready: false,
        budget_time: Some("1h".to_string()),
        budget_tokens: Some(1_000_000),
        budget_usd: Some(5.50),
        max_agents: Some(4),
        cost_tracker_path: None,
        terminal_criteria: Default::default(),
        delivery_policy: Default::default(),
        merge_policy: Default::default(),
        slice_execution: false,
        artifacts: Vec::new(),
        failure: None,
        state_dir: goal_dir.clone(),
    };

    store.save(&state).await.unwrap();
    let loaded = store.load(&goal_dir).await.unwrap();

    assert_eq!(loaded.goal_id, state.goal_id);
    assert_eq!(loaded.original_goal, state.original_goal);
    assert_eq!(loaded.normalized_goal, state.normalized_goal);
    assert_eq!(loaded.status, state.status);
    assert_eq!(loaded.phase, state.phase);
    assert_eq!(loaded.until_ready, state.until_ready);
    assert_eq!(loaded.budget_time, state.budget_time);
    assert_eq!(loaded.budget_tokens, state.budget_tokens);
    assert_eq!(loaded.budget_usd, state.budget_usd);
    assert_eq!(loaded.max_agents, state.max_agents);
    assert_eq!(loaded.state_dir, goal_dir);
}

#[tokio::test]
async fn db_goal_state_store_list_returns_goals_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("goals.db");
    let db = crate::runtime::db::DbHandle::open(&db_path).await.unwrap();
    let store = DbGoalStateStore::new(db);

    let t1 = chrono::Utc::now();
    let t2 = t1 + chrono::Duration::seconds(10);

    let goal_a = GoalState {
        version: 1,
        goal_id: "goal-a".to_string(),
        original_goal: "Goal A".to_string(),
        normalized_goal: "goal a".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        created_at: t1,
        updated_at: t1,
        completed_at: None,
        until_ready: false,
        budget_time: None,
        budget_tokens: None,
        budget_usd: None,
        max_agents: None,
        cost_tracker_path: None,
        terminal_criteria: Default::default(),
        delivery_policy: Default::default(),
        merge_policy: Default::default(),
        slice_execution: false,
        artifacts: Vec::new(),
        failure: None,
        state_dir: dir.path().join("goal-a"),
    };

    let goal_b = GoalState {
        version: 1,
        goal_id: "goal-b".to_string(),
        original_goal: "Goal B".to_string(),
        normalized_goal: "goal b".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        created_at: t2,
        updated_at: t2,
        completed_at: None,
        until_ready: false,
        budget_time: None,
        budget_tokens: None,
        budget_usd: None,
        max_agents: None,
        cost_tracker_path: None,
        terminal_criteria: Default::default(),
        delivery_policy: Default::default(),
        merge_policy: Default::default(),
        slice_execution: false,
        artifacts: Vec::new(),
        failure: None,
        state_dir: dir.path().join("goal-b"),
    };

    store.save(&goal_a).await.unwrap();
    store.save(&goal_b).await.unwrap();

    let goals = store.list().await.unwrap();
    assert_eq!(goals.len(), 2);
    assert_eq!(goals[0].goal_id, "goal-b");
    assert_eq!(goals[1].goal_id, "goal-a");
}

#[tokio::test]
async fn db_full_goal_persistence_roundtrip() {
    use crate::runtime::db::types::BudgetCheckpoint;
    use crate::runtime::db::{BudgetRepo, EventRepo};
    use crate::runtime::gates::GateResult;
    use crate::runtime::goal::proof::GoalProof;
    use crate::runtime::goal::state::db_store::{load_proof_from_db, load_task_graph_from_db};
    use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("goals.db");
    let db = crate::runtime::db::DbHandle::open(&db_path).await.unwrap();

    let goal_dir = dir.path().join("goal-roundtrip");
    let store = DbGoalStateStore::new(db.clone());

    let now = chrono::Utc::now();
    let state = GoalState {
        version: 1,
        goal_id: "goal-roundtrip".to_string(),
        original_goal: "Roundtrip test".to_string(),
        normalized_goal: "roundtrip test".to_string(),
        status: GoalStatus::Running,
        phase: GoalPhase::Execution,
        created_at: now,
        updated_at: now,
        completed_at: None,
        until_ready: false,
        budget_time: Some("1h".to_string()),
        budget_tokens: Some(1_000_000),
        budget_usd: Some(5.50),
        max_agents: Some(4),
        cost_tracker_path: None,
        terminal_criteria: Default::default(),
        delivery_policy: Default::default(),
        merge_policy: Default::default(),
        slice_execution: false,
        artifacts: Vec::new(),
        failure: None,
        state_dir: goal_dir.clone(),
    };

    // Save goal state
    store.save(&state).await.unwrap();

    // Save task graph
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "goal-roundtrip".to_string(),
        generated_at: now,
        tasks: vec![GoalTask {
            id: "task-1".to_string(),
            title: "Task 1".to_string(),
            description: "Do something".to_string(),
            status: GoalTaskStatus::Pending,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 3,
            lease_expires_at: None,
            dependencies: Vec::new(),
            read_set: vec!["src/lib.rs".to_string()],
            write_set: vec!["src/lib.rs".to_string()],
            risk: "low".to_string(),
            acceptance: vec!["works".to_string()],
        }],
    };
    crate::runtime::goal::state::db_store::save_task_graph_to_db(&db, &task_graph)
        .await
        .unwrap();

    // Save proof
    let proof = GoalProof {
        version: 1,
        goal_id: "goal-roundtrip".to_string(),
        status: GoalStatus::NotReady,
        readiness: "not ready".to_string(),
        summary: "Test proof".to_string(),
        generated_at: now,
        artifacts: Vec::new(),
        task_graph_summary: Default::default(),
        changed_files: vec!["src/lib.rs".to_string()],
        commits: vec!["abc123".to_string()],
        git: None,
        gates: vec![GateResult {
            name: "compile".to_string(),
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
        }],
        post_mutation_gates_ran: false,
        known_gaps: vec![],
        human_decisions_required: vec![],
        recovery_status: None,
    };
    crate::runtime::goal::state::db_store::save_proof_to_db(&db, &proof)
        .await
        .unwrap();

    // Save event directly via EventRepo (bypasses global_db)
    db.event_repo()
        .append("goal-roundtrip", "RunStarted", "{}")
        .await
        .unwrap();

    // Save budget checkpoint directly via BudgetRepo (bypasses global_db)
    let checkpoint = BudgetCheckpoint {
        checkpoint_id: None,
        goal_id: "goal-roundtrip".to_string(),
        version: 1,
        label: "test_checkpoint".to_string(),
        status: GoalStatus::Running.to_string(),
        phase: GoalPhase::Execution.to_string(),
        recorded_at: now.timestamp(),
        budget_time: Some("1h".to_string()),
        total_budget_secs: Some(3600),
        elapsed_since_created_secs: 60,
        remaining_budget_secs: Some(3540),
        budget_tokens: Some(1_000_000),
        used_tokens: 1000,
        remaining_budget_tokens: Some(999_000),
        budget_usd: Some(550),
        estimated_cost_usd: 50,
        remaining_budget_usd: Some(500),
        limit_value: None,
        used_value: None,
        created_at: now.timestamp(),
    };
    db.budget_repo()
        .append_checkpoint(&checkpoint)
        .await
        .unwrap();

    // Load everything back
    let loaded_state = store.load(&goal_dir).await.unwrap();
    let loaded_graph = load_task_graph_from_db(&db, "goal-roundtrip")
        .await
        .unwrap()
        .expect("task graph should exist");
    let loaded_proof = load_proof_from_db(&db, "goal-roundtrip")
        .await
        .unwrap()
        .expect("proof should exist");
    let loaded_events = db
        .event_repo()
        .get_by_goal("goal-roundtrip", None, None)
        .await
        .unwrap();
    let loaded_checkpoints = db
        .budget_repo()
        .get_by_goal("goal-roundtrip")
        .await
        .unwrap();

    // Assertions
    assert_eq!(loaded_state.goal_id, state.goal_id);
    assert_eq!(loaded_graph.tasks.len(), 1);
    assert_eq!(loaded_graph.tasks[0].id, "task-1");
    assert_eq!(loaded_proof.changed_files, vec!["src/lib.rs"]);
    assert_eq!(loaded_proof.gates.len(), 1);
    assert!(loaded_proof.gates[0].passed);
    assert_eq!(loaded_events.len(), 1);
    assert_eq!(loaded_checkpoints.len(), 1);
}
