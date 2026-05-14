use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn xdg_state(envs: &[(&str, PathBuf)]) -> PathBuf {
    envs.iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.clone()))
        .expect("missing XDG_STATE_HOME")
}

fn set_envs(envs: &[(&str, PathBuf)]) {
    for (key, value) in envs {
        std::env::set_var(key, value);
    }
}

fn unset_envs(envs: &[(&str, PathBuf)]) {
    for (key, _) in envs {
        std::env::remove_var(key);
    }
}

fn create_goal_scaffold(envs: &[(&str, PathBuf)]) -> (PathBuf, String) {
    let goals_dir = xdg_state(envs).join("omk").join("goals");
    fs::create_dir_all(&goals_dir).expect("create goals dir");

    let goal_id = "goal-recovery-test-01";
    let goal_dir = goals_dir.join(goal_id);
    fs::create_dir_all(&goal_dir).expect("create goal dir");

    fs::write(
        goal_dir.join("goal.json"),
        json!({
            "version": 1,
            "goal_id": goal_id,
            "original_goal": "Test recovery",
            "normalized_goal": "Test recovery",
            "status": "not_ready",
            "phase": "intake",
            "created_at": "2026-05-13T00:00:00Z",
            "updated_at": "2026-05-13T00:00:01Z",
            "until_ready": false,
            "terminal_criteria": {
                "proof_required": true,
                "gates_required": true,
                "human_blockers_stop": true
            },
            "artifacts": []
        })
        .to_string(),
    )
    .expect("write goal state");

    fs::write(
        goal_dir.join("task-graph.json"),
        json!({
            "version": 1,
            "goal_id": goal_id,
            "generated_at": "2026-05-13T00:00:00Z",
            "tasks": [
                {
                    "id": "goal-intake",
                    "title": "Intake",
                    "description": "Intake task",
                    "status": "done",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": [],
                    "risk": "low",
                    "acceptance": ["Intake done"]
                }
            ]
        })
        .to_string(),
    )
    .expect("write task graph");

    fs::write(
        goal_dir.join("proof.json"),
        json!({
            "version": 1,
            "goal_id": goal_id,
            "status": "not_ready",
            "readiness": "not ready: controller scaffold",
            "summary": "Test proof",
            "generated_at": "2026-05-13T00:00:00Z",
            "artifacts": [],
            "task_graph_summary": {
                "total_tasks": 1,
                "pending_tasks": 0,
                "blocked_tasks": 0,
                "done_tasks": 1
            },
            "changed_files": [],
            "commits": [],
            "gates": [],
            "post_mutation_gates_ran": false,
            "known_gaps": [],
            "human_decisions_required": []
        })
        .to_string(),
    )
    .expect("write proof");

    let events = vec![
        json!({
            "id": "evt-1",
            "run_id": goal_id,
            "ts": "2026-05-13T00:00:01Z",
            "schema_version": 1,
            "kind": "task_completed",
            "actor": "goal-controller",
            "payload": {"task_id": "goal-intake"}
        }),
        json!({
            "id": "evt-2",
            "run_id": goal_id,
            "ts": "2026-05-13T00:00:02Z",
            "schema_version": 1,
            "kind": "goal_created",
            "actor": "goal-controller",
            "payload": {}
        }),
    ];
    let content = events
        .into_iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(goal_dir.join("events.jsonl"), content).expect("write events");

    (goal_dir, goal_id.to_string())
}

#[tokio::test]
async fn replay_after_partial_event_log_does_not_crash_and_surfaces_recovery_status() {
    let _guard = ENV_LOCK.lock().await;
    let (tmp, envs) = isolated_env();
    set_envs(&envs);

    let (goal_dir, goal_id) = create_goal_scaffold(&envs);

    // Append a partial/corrupted line to the event log.
    let event_log = goal_dir.join("events.jsonl");
    let mut content = fs::read_to_string(&event_log).expect("read events");
    content.push_str("\n{\"id\":\"partial\", \"run_id\":\"");
    fs::write(&event_log, content).expect("append partial event");

    // Replay must succeed despite the trailing garbage.
    let replay = omk::runtime::goal::replay_goal(&goal_id)
        .await
        .expect("replay should not crash on partial event log");

    assert!(
        replay.recovery_status.is_some(),
        "recovery_status should be set for partial event log"
    );
    assert!(
        replay.recovery_status.as_ref().unwrap().contains("partial"),
        "recovery_status should mention 'partial'"
    );
    assert!(
        replay.parse_failures > 0,
        "parse_failures should be > 0 for partial event log"
    );
    assert!(
        replay.known_gaps.iter().any(|g| g.contains("malformed")),
        "known_gaps should mention malformed lines"
    );

    unset_envs(&envs);
    drop(tmp);
}

#[tokio::test]
async fn duplicate_events_do_not_break_replay_and_are_collapsed() {
    let _guard = ENV_LOCK.lock().await;
    let (tmp, envs) = isolated_env();
    set_envs(&envs);

    let (goal_dir, goal_id) = create_goal_scaffold(&envs);

    // Inject an exact duplicate of the first event.
    let event_log = goal_dir.join("events.jsonl");
    let lines: Vec<String> = fs::read_to_string(&event_log)
        .expect("read events")
        .lines()
        .map(String::from)
        .collect();
    assert!(!lines.is_empty(), "event log should have at least one line");
    let mut content = lines.join("\n");
    content.push('\n');
    content.push_str(&lines[0]);
    fs::write(&event_log, content).expect("write events with duplicate");

    let replay = omk::runtime::goal::replay_goal(&goal_id)
        .await
        .expect("replay should not crash with duplicates");

    assert!(
        replay.duplicate_events > 0,
        "duplicate_events should be > 0"
    );
    assert!(
        replay.known_gaps.iter().any(|g| g.contains("duplicate")),
        "known_gaps should mention duplicates"
    );

    // Idempotency: running replay again should yield the same event count.
    let replay2 = omk::runtime::goal::replay_goal(&goal_id)
        .await
        .expect("second replay should succeed");
    assert_eq!(
        replay.event_count, replay2.event_count,
        "replay should be idempotent: same event count"
    );
    assert_eq!(
        replay.duplicate_events, replay2.duplicate_events,
        "replay should be idempotent: same duplicate count"
    );

    unset_envs(&envs);
    drop(tmp);
}

#[tokio::test]
async fn missing_optional_task_graph_surfaces_known_gap_in_replay() {
    let _guard = ENV_LOCK.lock().await;
    let (tmp, envs) = isolated_env();
    set_envs(&envs);

    let (goal_dir, goal_id) = create_goal_scaffold(&envs);

    // Remove the task graph (optional for replay).
    fs::remove_file(goal_dir.join("task-graph.json")).expect("remove task graph");

    let replay = omk::runtime::goal::replay_goal(&goal_id)
        .await
        .expect("replay should not crash without task graph");

    assert!(
        replay.known_gaps.iter().any(|g| g.contains("Task graph")),
        "known_gaps should mention missing task graph"
    );
    assert_eq!(
        replay.task_graph_summary.total_tasks, 0,
        "task graph summary should be default/empty"
    );

    unset_envs(&envs);
    drop(tmp);
}

#[tokio::test]
async fn corrupted_required_state_returns_typed_actionable_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let goal_dir = tmp.path().join("goal-corrupted");
    fs::create_dir_all(&goal_dir).expect("create goal dir");

    // Corrupt the required goal.json file.
    fs::write(goal_dir.join("goal.json"), "this is not json").expect("corrupt goal state");

    let err = omk::runtime::goal::GoalState::load(&goal_dir)
        .await
        .expect_err("should fail on corrupted state");

    let typed = err.downcast_ref::<omk::runtime::goal::GoalStateError>();
    assert!(
        typed.is_some(),
        "error should be downcastable to GoalStateError, got: {err}"
    );
    match typed.unwrap() {
        omk::runtime::goal::GoalStateError::InvalidFormat { path, reason } => {
            assert!(path.contains("goal.json"), "path should mention goal.json");
            assert!(!reason.is_empty(), "reason should not be empty");
        }
        other => panic!("expected InvalidFormat, got: {other:?}"),
    }
}

#[tokio::test]
async fn missing_optional_proof_rebuilds_scaffold_with_recovery_status() {
    let _guard = ENV_LOCK.lock().await;
    let (tmp, envs) = isolated_env();
    set_envs(&envs);

    let (goal_dir, goal_id) = create_goal_scaffold(&envs);

    // Remove the proof file.
    fs::remove_file(goal_dir.join("proof.json")).expect("remove proof");

    let proof = omk::runtime::goal::resolve_goal_proof(&goal_id)
        .await
        .expect("resolve_goal_proof should rebuild from state when proof is missing");

    assert!(
        proof.recovery_status.is_some(),
        "recovery_status should be set when proof is rebuilt"
    );
    assert!(
        proof
            .recovery_status
            .as_ref()
            .unwrap()
            .contains("recovered"),
        "recovery_status should mention 'recovered'"
    );
    assert!(
        proof.known_gaps.iter().any(|g| g.contains("Proof file")),
        "known_gaps should mention missing proof"
    );

    unset_envs(&envs);
    drop(tmp);
}

#[tokio::test]
async fn missing_required_state_returns_missing_file_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let goal_dir = tmp.path().join("goal-missing");
    fs::create_dir_all(&goal_dir).expect("create goal dir");

    // Do NOT write goal.json.

    let err = omk::runtime::goal::GoalState::load(&goal_dir)
        .await
        .expect_err("should fail on missing state");

    let typed = err.downcast_ref::<omk::runtime::goal::GoalStateError>();
    assert!(
        typed.is_some(),
        "error should be downcastable to GoalStateError, got: {err}"
    );
    match typed.unwrap() {
        omk::runtime::goal::GoalStateError::MissingFile { path } => {
            assert!(path.contains("goal.json"), "path should mention goal.json");
        }
        other => panic!("expected MissingFile, got: {other:?}"),
    }
}

#[tokio::test]
async fn unreadable_required_state_returns_io_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let goal_dir = tmp.path().join("goal-unreadable");
    fs::create_dir_all(&goal_dir).expect("create goal dir");

    // Write goal.json as a directory (causes read_to_string to fail with IsADirectory).
    fs::create_dir(goal_dir.join("goal.json")).expect("create goal.json as directory");

    let err = omk::runtime::goal::GoalState::load(&goal_dir)
        .await
        .expect_err("should fail on unreadable state");

    let typed = err.downcast_ref::<omk::runtime::goal::GoalStateError>();
    assert!(
        typed.is_some(),
        "error should be downcastable to GoalStateError, got: {err}"
    );
    match typed.unwrap() {
        omk::runtime::goal::GoalStateError::IoError { path, reason } => {
            assert!(path.contains("goal.json"), "path should mention goal.json");
            assert!(!reason.is_empty(), "reason should not be empty");
        }
        other => panic!("expected IoError, got: {other:?}"),
    }
}

#[test]
fn proof_recovery_status_roundtrips_through_json() {
    let proof = omk::runtime::goal::GoalProof {
        version: 1,
        goal_id: "goal-roundtrip".to_string(),
        status: omk::runtime::goal::GoalStatus::NotReady,
        readiness: "not ready".to_string(),
        summary: "roundtrip test".to_string(),
        generated_at: chrono::Utc::now(),
        artifacts: Vec::new(),
        task_graph_summary: omk::runtime::goal::GoalTaskGraphSummary {
            total_tasks: 1,
            pending_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 1,
        },
        changed_files: Vec::new(),
        commits: Vec::new(),
        git: None,
        gates: Vec::new(),
        post_mutation_gates_ran: false,
        known_gaps: vec!["test gap".to_string()],
        human_decisions_required: Vec::new(),
        recovery_status: Some("recovered: test".to_string()),
    };

    let json = serde_json::to_string_pretty(&proof).expect("serialize proof");
    let deserialized: omk::runtime::goal::GoalProof =
        serde_json::from_str(&json).expect("deserialize proof");

    assert_eq!(
        deserialized.recovery_status,
        Some("recovered: test".to_string())
    );
    assert_eq!(deserialized.known_gaps, vec!["test gap".to_string()]);
    assert_eq!(deserialized.goal_id, "goal-roundtrip");
}
