use chrono::Utc;
use omk::runtime::goal::{
    write_rejection_rollback_plan, GoalPhase, GoalProof, GoalState, GoalStatus,
    GoalTaskGraphSummary,
};
use std::path::PathBuf;

fn test_state(state_dir: PathBuf) -> GoalState {
    GoalState {
        version: 1,
        goal_id: "goal-rollback-test".to_string(),
        original_goal: "test rollback".to_string(),
        normalized_goal: "test rollback".to_string(),
        status: GoalStatus::NotReady,
        phase: GoalPhase::Proof,
        created_at: Utc::now(),
        updated_at: Utc::now(),
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
        state_dir,
    }
}

fn test_proof() -> GoalProof {
    GoalProof {
        version: 1,
        goal_id: "goal-rollback-test".to_string(),
        status: GoalStatus::NotReady,
        readiness: "not ready".to_string(),
        summary: "test".to_string(),
        generated_at: Utc::now(),
        artifacts: Vec::new(),
        task_graph_summary: GoalTaskGraphSummary {
            total_tasks: 1,
            pending_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
        },
        changed_files: vec!["src/main.rs".to_string()],
        commits: Vec::new(),
        git: None,
        gates: Vec::new(),
        post_mutation_gates_ran: false,
        known_gaps: Vec::new(),
        human_decisions_required: Vec::new(),
        recovery_status: None,
    }
}

async fn write_task_graph(state_dir: &std::path::Path, delivery: serde_json::Value) {
    let graph = serde_json::json!({
        "version": 1,
        "goal_id": "goal-rollback-test",
        "generated_at": Utc::now(),
        "tasks": [
            {
                "id": "task-1",
                "title": "Task 1",
                "description": "Task 1 description",
                "status": "pending",
                "dependencies": [],
                "read_set": [],
                "write_set": [],
                "risk": "low",
                "acceptance": ["Task 1 acceptance"],
                "delivery": delivery
            }
        ]
    });
    tokio::fs::write(
        state_dir.join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE),
        serde_json::to_vec_pretty(&graph).unwrap(),
    )
    .await
    .unwrap();
}

async fn write_multi_task_graph(
    state_dir: &std::path::Path,
    deliveries: Vec<(&str, serde_json::Value)>,
) {
    let mut tasks = Vec::new();
    for (task_id, delivery) in deliveries {
        tasks.push(serde_json::json!({
            "id": task_id,
            "title": format!("Task {}", task_id),
            "description": format!("Task {} description", task_id),
            "status": "pending",
            "dependencies": [],
            "read_set": [],
            "write_set": [],
            "risk": "low",
            "acceptance": [format!("Task {} acceptance", task_id)],
            "delivery": delivery
        }));
    }
    let graph = serde_json::json!({
        "version": 1,
        "goal_id": "goal-rollback-test",
        "generated_at": Utc::now(),
        "tasks": tasks
    });
    tokio::fs::write(
        state_dir.join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE),
        serde_json::to_vec_pretty(&graph).unwrap(),
    )
    .await
    .unwrap();
}

fn rollback_path(state_dir: &std::path::Path) -> PathBuf {
    state_dir
        .join("artifacts")
        .join("integration")
        .join("rollback-rejected-slice.md")
}

async fn setup_integration_dir(state_dir: &std::path::Path) {
    tokio::fs::create_dir_all(state_dir.join("artifacts").join("integration"))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_rollback_without_conflict_metadata_matches_legacy_output() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    // Legacy path: no task graph with conflict delivery metadata.
    write_rejection_rollback_plan(&state, &proof, "review fail")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(!content.contains("## Conflict evidence"));
    assert!(content.contains("## Scope"));
    assert!(content.contains("src/main.rs"));
    assert!(content.contains("## Recovery Steps"));
}

#[tokio::test]
async fn test_rollback_includes_blocking_reason_when_present() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-a",
            "conflict_blocking_reason": "merge_tree_conflict_unresolvable"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(content.contains("## Conflict evidence — slice `slice-a`"));
    assert!(content.contains("**Blocking reason:** merge_tree_conflict_unresolvable"));
}

#[tokio::test]
async fn test_rollback_embeds_conflict_evidence_file_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    let evidence_path = state_dir
        .join("artifacts")
        .join("integration")
        .join("conflict.json");
    tokio::fs::create_dir_all(evidence_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&evidence_path, b"<<conflict>>")
        .await
        .unwrap();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-a",
            "conflict_evidence_path": "artifacts/integration/conflict.json"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(content.contains("**Evidence artifact:** `artifacts/integration/conflict.json`"));
    assert!(content.contains("<<conflict>>"));
    assert!(content.contains("```"));
}

#[tokio::test]
async fn test_rollback_handles_missing_conflict_evidence_file_gracefully() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-a",
            "conflict_evidence_path": "artifacts/integration/missing.json"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(content.contains("_(unreadable:"));
}

#[tokio::test]
async fn test_rollback_handles_oversized_evidence_with_truncation() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    let evidence_path = state_dir
        .join("artifacts")
        .join("integration")
        .join("big.conflict");
    tokio::fs::create_dir_all(evidence_path.parent().unwrap())
        .await
        .unwrap();
    let big_content = "x".repeat(100 * 1024);
    tokio::fs::write(&evidence_path, big_content.as_bytes())
        .await
        .unwrap();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-a",
            "conflict_evidence_path": "artifacts/integration/big.conflict"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(content.contains("...(truncated"));
    let fence_start = content.find("```\n").unwrap() + 4;
    let fence_end = content[fence_start..].find("```").unwrap() + fence_start;
    let embedded = &content[fence_start..fence_end];
    assert!(embedded.len() <= 64 * 1024 + 100);
}

#[tokio::test]
async fn test_rollback_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-a",
            "conflict_blocking_reason": "merge_tree_conflict_unresolvable"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();
    let first = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();
    let second = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();

    assert_eq!(first, second);
}

#[tokio::test]
async fn test_rollback_with_multiple_rejected_slices_sorted_alphabetically() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    write_multi_task_graph(
        &state_dir,
        vec![
            (
                "task-z",
                serde_json::json!({
                    "slice_id": "slice-z",
                    "conflict_blocking_reason": "z"
                }),
            ),
            (
                "task-a",
                serde_json::json!({
                    "slice_id": "slice-a",
                    "conflict_blocking_reason": "a"
                }),
            ),
            (
                "task-m",
                serde_json::json!({
                    "slice_id": "slice-m",
                    "conflict_blocking_reason": "m"
                }),
            ),
        ],
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    let pos_a = content.find("slice `slice-a`").unwrap();
    let pos_m = content.find("slice `slice-m`").unwrap();
    let pos_z = content.find("slice `slice-z`").unwrap();
    assert!(pos_a < pos_m);
    assert!(pos_m < pos_z);
}

#[tokio::test]
async fn test_rollback_handles_binary_evidence_via_lossy_utf8() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    let evidence_path = state_dir
        .join("artifacts")
        .join("integration")
        .join("binary.conflict");
    tokio::fs::create_dir_all(evidence_path.parent().unwrap())
        .await
        .unwrap();
    let mut binary = vec![0x80, 0x81, 0x82, 0xff, 0xfe];
    binary.extend_from_slice(b"hello");
    tokio::fs::write(&evidence_path, &binary).await.unwrap();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-a",
            "conflict_evidence_path": "artifacts/integration/binary.conflict"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(content.contains('\u{FFFD}')); // replacement char
    assert!(content.contains("hello"));
}


#[tokio::test]
async fn test_rollback_preserves_rebase_conflict_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().to_path_buf();
    setup_integration_dir(&state_dir).await;
    let state = test_state(state_dir.clone());
    let proof = test_proof();

    let evidence_path = state_dir
        .join("artifacts")
        .join("integration")
        .join("rebase-conflict.json");
    tokio::fs::create_dir_all(evidence_path.parent().unwrap())
        .await
        .unwrap();
    let evidence = serde_json::json!({
        "task_id": "task-rebase-fail",
        "source_ref": "feature",
        "target_ref": "master",
        "clean_merge": false,
        "conflicting_files": ["src/main.rs"],
        "command_line": "git merge-tree master feature",
        "stdout_summary": "src/main.rs",
        "stderr_summary": "",
        "artifact_path": "artifacts/integration/rebase-conflict.json",
        "conflict_classification": {
            "Unsafe": {
                "reason": "file 'src/main.rs' contains substantive conflicts"
            }
        }
    });
    tokio::fs::write(&evidence_path, serde_json::to_vec_pretty(&evidence).unwrap())
        .await
        .unwrap();

    write_task_graph(
        &state_dir,
        serde_json::json!({
            "slice_id": "slice-rebase",
            "conflict_evidence_path": "artifacts/integration/rebase-conflict.json",
            "conflict_blocking_reason": "auto-rebase could not resolve conflicts: file 'src/main.rs' contains substantive conflicts"
        }),
    )
    .await;

    write_rejection_rollback_plan(&state, &proof, "merge conflict")
        .await
        .unwrap();

    let content = tokio::fs::read_to_string(rollback_path(&state_dir))
        .await
        .unwrap();
    assert!(content.contains("## Conflict evidence — slice `slice-rebase`"));
    assert!(content.contains("**Blocking reason:** auto-rebase could not resolve conflicts"));
    assert!(content.contains("**Evidence artifact:** `artifacts/integration/rebase-conflict.json`"));
    assert!(content.contains("src/main.rs"));
    assert!(content.contains("Unsafe"));
    assert!(content.contains("substantive conflicts"));
}
