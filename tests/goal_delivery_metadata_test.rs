use assert_cmd::Command;
use chrono::Utc;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn omk_cmd(envs: &[(&'static str, PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd
}

fn xdg_state(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    envs.iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.clone()))
        .expect("missing XDG_STATE_HOME")
}

fn goal_dirs(envs: &[(&'static str, PathBuf)]) -> Vec<PathBuf> {
    let goals_dir = xdg_state(envs).join("omk").join("goals");
    let mut dirs: Vec<_> = fs::read_dir(goals_dir)
        .expect("missing goals dir")
        .map(|entry| entry.expect("failed to read goal entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn write_gate_config(project_dir: &std::path::Path) {
    let omk_dir = project_dir.join(".omk");
    fs::create_dir_all(&omk_dir).expect("failed to create .omk dir");
    fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "smoke"
command = "/bin/sh"
args = ["-c", "true"]
required = true
"#,
    )
    .expect("failed to write gates.toml");
}

fn delivery_metadata() -> Value {
    json!({
        "slice_id": "goal-agent-execute",
        "owner": "codex",
        "branch": "codex/goal-agent-execute-delivery-metadata",
        "pr_link": "https://github.com/ekhodzitsky/oh-my-kimi/pull/123",
        "write_scope": [
            "src/runtime/goal/task_graph.rs",
            "src/runtime/goal/proof.rs",
            "tests/goal_delivery_metadata_test.rs"
        ],
        "verification_summary": "cargo test --test goal_delivery_metadata_test passed"
    })
}

fn task_graph_json(delivery: Option<Value>) -> Value {
    let mut task = json!({
        "id": "goal-agent-execute",
        "title": "Execute delivery slice",
        "description": "Record delivery metadata for the implementation slice.",
        "status": "pending",
        "dependencies": [],
        "read_set": [],
        "write_set": ["src/runtime/goal/task_graph.rs"],
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

fn inject_delivery_metadata(task_graph_path: &Path) {
    let mut task_graph: Value =
        serde_json::from_slice(&fs::read(task_graph_path).expect("read generated task graph"))
            .expect("task graph json");
    let task = task_graph["tasks"]
        .as_array_mut()
        .expect("task graph tasks should be an array")
        .iter_mut()
        .find(|task| task["id"] == "goal-agent-execute")
        .expect("generated graph should include agent execution task");
    task["delivery"] = delivery_metadata();
    fs::write(
        task_graph_path,
        serde_json::to_vec_pretty(&task_graph).expect("task graph json"),
    )
    .expect("rewrite generated task graph");
}

fn task_delivery<'a>(task_graph: &'a Value, task_id: &str) -> &'a Value {
    task_graph["tasks"]
        .as_array()
        .expect("task graph tasks should be an array")
        .iter()
        .find(|task| task["id"] == task_id)
        .and_then(|task| task.get("delivery"))
        .expect("delivery metadata should persist on the task node")
}

fn proof_delivery<'a>(proof_json: &'a Value, task_id: &str) -> &'a Value {
    proof_json["delivery_metadata"]
        .as_array()
        .expect("proof should include delivery metadata array")
        .iter()
        .find(|delivery| delivery["task_id"] == task_id)
        .expect("proof should include task delivery metadata")
}

fn assert_delivery_metadata(delivery: &Value) {
    assert_eq!(delivery["slice_id"], "goal-agent-execute");
    assert_eq!(delivery["owner"], "codex");
    assert_eq!(
        delivery["branch"],
        "codex/goal-agent-execute-delivery-metadata"
    );
    assert_eq!(
        delivery["pr_link"],
        "https://github.com/ekhodzitsky/oh-my-kimi/pull/123"
    );
    assert_eq!(
        delivery["write_scope"]
            .as_array()
            .expect("write_scope should be an array")
            .len(),
        3
    );
    assert_eq!(
        delivery["verification_summary"],
        "cargo test --test goal_delivery_metadata_test passed"
    );
}

#[tokio::test]
async fn legacy_task_graph_loads_without_delivery_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE),
        serde_json::to_vec_pretty(&task_graph_json(None)).expect("task graph json"),
    )
    .expect("write legacy task graph");

    let graph = omk::runtime::goal::GoalTaskGraph::load(tmp.path())
        .await
        .expect("legacy task graph should load");
    let roundtrip = serde_json::to_value(&graph).expect("serialize task graph");

    assert!(roundtrip["tasks"][0].get("delivery").is_none());
}

#[tokio::test]
async fn task_graph_round_trips_populated_delivery_metadata() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("project tempdir");
    write_gate_config(project.path());

    let mut plan = omk_cmd(&envs);
    plan.current_dir(project.path())
        .args(["goal", "plan", "Round-trip delivery metadata"])
        .assert()
        .success();

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let task_graph_path = dirs[0].join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE);
    inject_delivery_metadata(&task_graph_path);

    let mut verify = omk_cmd(&envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    let roundtrip: Value =
        serde_json::from_slice(&fs::read(&task_graph_path).expect("read updated task graph"))
            .expect("updated task graph json");
    assert_delivery_metadata(task_delivery(&roundtrip, "goal-agent-execute"));
}

#[test]
fn public_goal_task_literal_remains_source_compatible() {
    let task = omk::runtime::goal::GoalTask {
        id: "goal-agent-execute".to_string(),
        title: "Execute delivery slice".to_string(),
        description: "Record delivery metadata for the implementation slice.".to_string(),
        status: omk::runtime::goal::GoalTaskStatus::Pending,
        owner_role: None,
        completed_at: None,
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: Vec::new(),
        read_set: Vec::new(),
        write_set: Vec::new(),
        risk: "medium".to_string(),
        acceptance: vec!["delivery metadata round-trips".to_string()],
    };

    assert_eq!(task.id, "goal-agent-execute");
}

#[test]
fn public_goal_proof_literal_remains_source_compatible() {
    let proof = omk::runtime::goal::GoalProof {
        version: 1,
        goal_id: "goal-delivery-test".to_string(),
        status: omk::runtime::goal::GoalStatus::NotReady,
        readiness: "not ready".to_string(),
        summary: "delivery metadata is sidecar JSON".to_string(),
        generated_at: Utc::now(),
        artifacts: Vec::new(),
        task_graph_summary: omk::runtime::goal::GoalTaskGraphSummary {
            total_tasks: 0,
            pending_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
        },
        changed_files: Vec::new(),
        commits: Vec::new(),
        git: None,
        gates: Vec::new(),
        post_mutation_gates_ran: false,
        known_gaps: Vec::new(),
        human_decisions_required: Vec::new(),
    };

    assert_eq!(proof.goal_id, "goal-delivery-test");
    let proof_json = serde_json::to_value(&proof).expect("proof json");
    assert!(proof_json.get("delivery_metadata").is_none());
}

#[tokio::test]
async fn proof_json_surfaces_task_delivery_metadata() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("project tempdir");
    write_gate_config(project.path());

    let mut plan = omk_cmd(&envs);
    plan.current_dir(project.path())
        .args(["goal", "plan", "Ship delivery metadata"])
        .assert()
        .success();

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let task_graph_path = dirs[0].join(omk::runtime::goal::GOAL_TASK_GRAPH_FILE);
    inject_delivery_metadata(&task_graph_path);

    let mut verify = omk_cmd(&envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    let proof_output = {
        let mut cmd = omk_cmd(&envs);
        cmd.current_dir(project.path())
            .args(["goal", "proof", "latest", "--json"])
            .output()
            .expect("omk goal proof failed")
    };
    assert!(proof_output.status.success());
    let proof_json: Value =
        serde_json::from_slice(&proof_output.stdout).expect("proof output should be JSON");
    assert_delivery_metadata(proof_delivery(&proof_json, "goal-agent-execute"));

    let loaded_proof = omk::runtime::goal::GoalProof::load(&dirs[0])
        .await
        .expect("goal proof should load");
    let loaded_proof_json = serde_json::to_value(&loaded_proof).expect("loaded proof json");
    assert_delivery_metadata(proof_delivery(&loaded_proof_json, "goal-agent-execute"));
}
