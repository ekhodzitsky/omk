use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::fs;
use std::time::Duration;

fn write_run_events(run_dir: &std::path::Path, run_id: &str) {
    let events = format!(
        r#"{{"id":"e1","run_id":"{run_id}","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}}
{{"id":"e2","run_id":"{run_id}","ts":"2024-01-01T00:00:01Z","schema_version":1,"kind":"worker_started","actor":"worker-1"}}
{{"id":"e3","run_id":"{run_id}","ts":"2024-01-01T00:00:02Z","schema_version":1,"kind":"task_started","actor":"worker-1","payload":{{"task_id":"task-1"}}}}
{{"id":"e4","run_id":"{run_id}","ts":"2024-01-01T00:00:03Z","schema_version":1,"kind":"task_completed","actor":"worker-1","payload":{{"task_id":"task-1"}}}}
{{"id":"e5","run_id":"{run_id}","ts":"2024-01-01T00:00:04Z","schema_version":1,"kind":"task_started","actor":"worker-2","payload":{{"task_id":"task-2"}}}}
{{"id":"e6","run_id":"{run_id}","ts":"2024-01-01T00:00:05Z","schema_version":1,"kind":"task_completed","actor":"worker-2","payload":{{"task_id":"task-2"}}}}
{{"id":"e7","run_id":"{run_id}","ts":"2024-01-01T00:00:06Z","schema_version":1,"kind":"run_completed"}}
"#
    );
    fs::write(run_dir.join("events.jsonl"), events).unwrap();
}

fn create_run_dir(envs: &[(&'static str, std::path::PathBuf)], run_id: &str) -> std::path::PathBuf {
    let xdg_state = envs
        .iter()
        .find(|(key, _)| *key == "XDG_STATE_HOME")
        .map(|(_, value)| value.clone())
        .unwrap();
    let runs_dir = xdg_state.join("omk").join("runs");
    let run_dir = runs_dir.join(run_id);
    fs::create_dir_all(&run_dir).unwrap();
    write_run_events(&run_dir, run_id);
    run_dir
}

fn setup_run_dir(run_id: &str) -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    let (tmp, envs) = omk::test_helpers::isolated_xdg_env();
    create_run_dir(&envs, run_id);
    (tmp, envs)
}

#[test]
fn test_run_show_filters_by_worker_task_kind() {
    let (_tmp, envs) = setup_run_dir("filter-run");

    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    cmd.arg("run")
        .arg("show")
        .arg("filter-run")
        .arg("--worker")
        .arg("worker-1")
        .arg("--task")
        .arg("task-1")
        .arg("--kind")
        .arg("TASK_COMPLETED");

    cmd.assert()
        .success()
        .stdout(contains("📋 Run timeline — filter-run (1 events)"))
        .stdout(contains("task_completed"))
        .stdout(contains("actor=worker-1"))
        .stdout(contains("task=task-1"))
        .stdout(predicates::str::contains("worker-2").not())
        .stdout(predicates::str::contains("task-2").not());
}

#[test]
fn test_run_show_json_shortcut() {
    let (_tmp, envs) = setup_run_dir("json-run");

    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    cmd.arg("run")
        .arg("show")
        .arg("json-run")
        .arg("--json")
        .arg("--worker")
        .arg("worker-1");

    cmd.assert()
        .success()
        .stdout(contains("\"kind\": \"worker_started\""))
        .stdout(contains("\"actor\": \"worker-1\""))
        .stdout(predicates::str::contains("📋 Run timeline").not());
}

#[test]
fn test_run_show_latest_resolves_most_recent_scheduler_run() {
    let (_tmp, envs) = omk::test_helpers::isolated_xdg_env();
    create_run_dir(&envs, "older-run");
    std::thread::sleep(Duration::from_millis(30));
    create_run_dir(&envs, "newer-run");

    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    cmd.arg("run")
        .arg("show")
        .arg("latest")
        .arg("--kind")
        .arg("run_completed");

    cmd.assert()
        .success()
        .stdout(contains("📋 Run timeline — newer-run (1 events)"))
        .stdout(contains("run_completed"))
        .stdout(predicates::str::contains("older-run").not());
}
