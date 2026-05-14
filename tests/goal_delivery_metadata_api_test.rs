use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn task_graph_json(delivery: Option<Value>) -> Value {
    let mut task = json!({
        "id": "goal-agent-execute",
        "title": "Execute delivery slice",
        "description": "Record delivery metadata for the implementation slice.",
        "status": "pending",
        "dependencies": [],
        "read_set": [],
        "write_set": ["src/runtime/goal/task_graph/delivery.rs"],
        "risk": "medium",
        "acceptance": ["delivery metadata round-trips"]
    });
    if let Some(delivery) = delivery {
        task["delivery"] = delivery;
    }

    json!({
        "version": 1,
        "goal_id": "goal-delivery-test",
        "generated_at": "2026-05-13T00:00:00Z",
        "tasks": [task]
    })
}

fn write_task_graph(delivery: Option<Value>) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let task_graph_path = tmp.path().join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE);
    fs::write(
        &task_graph_path,
        serde_json::to_vec_pretty(&task_graph_json(delivery)).expect("task graph json"),
    )
    .expect("write task graph");
    (tmp, task_graph_path)
}

fn read_task_graph(path: &PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read task graph")).expect("task graph json")
}

#[tokio::test]
async fn typed_api_creates_updates_and_merges_delivery_metadata() {
    let (tmp, task_graph_path) = write_task_graph(None);

    let created = omk::runtime::goal::update_goal_task_delivery_metadata(
        tmp.path(),
        "goal-agent-execute",
        omk::runtime::goal::GoalTaskDeliveryMetadataUpdate {
            owner: Some("codex".to_string()),
            write_scope: Some(vec![
                "src/runtime/goal/task_graph/delivery.rs".to_string(),
                "tests/goal_delivery_metadata_api_test.rs".to_string(),
            ]),
            branch: Some("codex/goal-delivery-metadata-api".to_string()),
            worktree_path: Some(PathBuf::from("../oh-my-kimi-goal-delivery")),
            status: Some(omk::runtime::goal::GoalTaskDeliveryStatus::InProgress),
            ..Default::default()
        },
    )
    .await
    .expect("create delivery metadata");

    assert_eq!(created.owner.as_deref(), Some("codex"));
    assert_eq!(
        created.branch.as_deref(),
        Some("codex/goal-delivery-metadata-api")
    );
    assert_eq!(created.write_scope.len(), 2);
    assert_eq!(
        created.status,
        Some(omk::runtime::goal::GoalTaskDeliveryStatus::InProgress)
    );

    let update = omk::runtime::goal::GoalTaskDeliveryMetadataUpdate {
        pr_url: Some("https://github.com/ekhodzitsky/oh-my-kimi/pull/123".to_string()),
        commit_sha: Some("abc1234".to_string()),
        verification_summary: Some(
            "cargo test --test goal_delivery_metadata_api_test passed".to_string(),
        ),
        status: Some(omk::runtime::goal::GoalTaskDeliveryStatus::Delivered),
        ..Default::default()
    };
    let merged = omk::runtime::goal::update_goal_task_delivery_metadata(
        tmp.path(),
        "goal-agent-execute",
        update.clone(),
    )
    .await
    .expect("merge delivery metadata");

    assert_eq!(merged.owner.as_deref(), Some("codex"));
    assert_eq!(
        merged.branch.as_deref(),
        Some("codex/goal-delivery-metadata-api")
    );
    assert_eq!(
        merged.pr_url.as_deref(),
        Some("https://github.com/ekhodzitsky/oh-my-kimi/pull/123")
    );
    assert_eq!(merged.commit_sha.as_deref(), Some("abc1234"));
    assert_eq!(
        merged.status,
        Some(omk::runtime::goal::GoalTaskDeliveryStatus::Delivered)
    );

    let before_repeat = fs::read_to_string(&task_graph_path).expect("read task graph");
    let repeated = omk::runtime::goal::update_goal_task_delivery_metadata(
        tmp.path(),
        "goal-agent-execute",
        update,
    )
    .await
    .expect("repeat merge delivery metadata");
    let after_repeat = fs::read_to_string(&task_graph_path).expect("read task graph");

    assert_eq!(merged, repeated);
    assert_eq!(before_repeat, after_repeat);

    let task_graph = read_task_graph(&task_graph_path);
    let task = &task_graph["tasks"][0];
    assert_eq!(task["title"], "Execute delivery slice");
    assert_eq!(
        task["write_set"][0],
        "src/runtime/goal/task_graph/delivery.rs"
    );
    assert_eq!(task["delivery"]["owner"], "codex");
    assert_eq!(task["delivery"]["commit_sha"], "abc1234");
}

#[tokio::test]
async fn typed_api_preserves_unknown_and_legacy_delivery_fields() {
    let legacy_delivery = json!({
        "owner": "codex",
        "task_id": "legacy-shadow",
        "write_scope": "legacy-string-shape",
        "legacy_ticket": "OMK-123",
        "pr_link": "https://github.com/ekhodzitsky/oh-my-kimi/pull/99"
    });
    let (tmp, task_graph_path) = write_task_graph(Some(legacy_delivery));

    let metadata = omk::runtime::goal::update_goal_task_delivery_metadata(
        tmp.path(),
        "goal-agent-execute",
        omk::runtime::goal::GoalTaskDeliveryMetadataUpdate {
            commit_sha: Some("def5678".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("update legacy delivery metadata");

    assert_eq!(metadata.owner.as_deref(), Some("codex"));
    assert!(metadata.write_scope.is_empty());
    assert_eq!(metadata.extra["task_id"], "legacy-shadow");
    assert_eq!(metadata.extra["legacy_ticket"], "OMK-123");
    assert_eq!(
        metadata.extra["pr_link"],
        "https://github.com/ekhodzitsky/oh-my-kimi/pull/99"
    );

    let task_graph = read_task_graph(&task_graph_path);
    let delivery = &task_graph["tasks"][0]["delivery"];
    assert_eq!(delivery["task_id"], "legacy-shadow");
    assert_eq!(delivery["legacy_ticket"], "OMK-123");
    assert_eq!(delivery["write_scope"], "legacy-string-shape");
    assert_eq!(
        delivery["pr_link"],
        "https://github.com/ekhodzitsky/oh-my-kimi/pull/99"
    );
    assert_eq!(delivery["commit_sha"], "def5678");

    let records = omk::runtime::goal::load_goal_task_delivery_records(tmp.path())
        .await
        .expect("load delivery records");
    let record_json = serde_json::to_value(&records[0]).expect("record json");
    assert_eq!(record_json["task_id"], "goal-agent-execute");
    assert_eq!(record_json["legacy_ticket"], "OMK-123");
}

#[tokio::test]
async fn typed_api_updates_only_target_task_delivery_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let task_graph_path = tmp.path().join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE);
    let task_graph = json!({
        "version": 1,
        "goal_id": "goal-delivery-test",
        "generated_at": "2026-05-13T00:00:00Z",
        "tasks": [
            {
                "id": "goal-agent-execute",
                "title": "Execute delivery slice",
                "description": "Record delivery metadata for the implementation slice.",
                "status": "pending",
                "dependencies": [],
                "read_set": [],
                "write_set": ["src/runtime/goal/task_graph/delivery.rs"],
                "risk": "medium",
                "acceptance": ["delivery metadata round-trips"]
            },
            {
                "id": "goal-review",
                "title": "Review delivery slice",
                "description": "Review the implementation delivery evidence.",
                "status": "pending",
                "dependencies": ["goal-agent-execute"],
                "read_set": ["src/runtime/goal/task_graph/delivery.rs"],
                "write_set": [],
                "risk": "low",
                "acceptance": ["review evidence remains attached"],
                "delivery": {
                    "owner": "reviewer",
                    "branch": "review/existing",
                    "legacy_ticket": "OMK-456"
                }
            }
        ]
    });
    fs::write(
        &task_graph_path,
        serde_json::to_vec_pretty(&task_graph).expect("task graph json"),
    )
    .expect("write task graph");
    let untouched_task = task_graph["tasks"][1].clone();

    omk::runtime::goal::update_goal_task_delivery_metadata(
        tmp.path(),
        "goal-agent-execute",
        omk::runtime::goal::GoalTaskDeliveryMetadataUpdate {
            owner: Some("codex".to_string()),
            branch: Some("codex/goal-delivery-metadata-api".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("update target delivery metadata");

    let updated = read_task_graph(&task_graph_path);
    assert_eq!(updated["tasks"][1], untouched_task);
    assert_eq!(updated["tasks"][0]["title"], "Execute delivery slice");
    assert_eq!(updated["tasks"][0]["delivery"]["owner"], "codex");
}

#[tokio::test]
async fn typed_api_leaves_tasks_without_delivery_metadata_compatible() {
    let (tmp, _task_graph_path) = write_task_graph(None);

    let metadata =
        omk::runtime::goal::read_goal_task_delivery_metadata(tmp.path(), "goal-agent-execute")
            .await
            .expect("read delivery metadata");
    let graph = omk::runtime::goal::GoalTaskGraph::load(tmp.path())
        .await
        .expect("legacy task graph should load");

    assert!(metadata.is_none());
    assert_eq!(graph.tasks[0].id, "goal-agent-execute");
}
