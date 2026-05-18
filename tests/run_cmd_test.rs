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
#[ignore = "flaky: uses thread::sleep for filesystem timing (#TODO)"]
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

#[test]
fn test_run_show_text_displays_wire_evidence_fields_without_raw_dump() {
    let (_tmp, envs) = omk::test_helpers::isolated_xdg_env();
    let xdg_state = envs
        .iter()
        .find(|(key, _)| *key == "XDG_STATE_HOME")
        .map(|(_, value)| value.clone())
        .unwrap();
    let run_dir = xdg_state.join("omk").join("runs").join("wire-run");
    fs::create_dir_all(&run_dir).unwrap();
    let events = r#"{"id":"e1","run_id":"wire-run","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}
{"id":"e2","run_id":"wire-run","ts":"2024-01-01T00:00:01Z","schema_version":1,"kind":"task_output","actor":"worker-1","payload":{"task_id":"task-1","wire_event":"turn_delta","wire_method":"prompt","message":"chunk","reason":"stream","wire_request_id":"req-1","wire_request":"review","output_summary":"done","request_payload":{"noise":"x"}}}
{"id":"e3","run_id":"wire-run","ts":"2024-01-01T00:00:02Z","schema_version":1,"kind":"run_completed"}
"#;
    fs::write(run_dir.join("events.jsonl"), events).unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    cmd.arg("run").arg("show").arg("wire-run");
    cmd.assert()
        .success()
        .stdout(contains("wire_event=turn_delta"))
        .stdout(contains("wire_method=prompt"))
        .stdout(contains("wire_request=review"))
        .stdout(contains("wire_request_id=req-1"))
        .stdout(contains("output_summary=done"))
        .stdout(contains("message=chunk"))
        .stdout(contains("reason=stream"))
        .stdout(predicates::str::contains("request_payload").not());
}

#[test]
fn test_run_show_reads_legacy_event_log_alias_when_canonical_absent() {
    let (_tmp, envs) = omk::test_helpers::isolated_xdg_env();
    let xdg_state = envs
        .iter()
        .find(|(key, _)| *key == "XDG_STATE_HOME")
        .map(|(_, value)| value.clone())
        .unwrap();
    let run_dir = xdg_state.join("omk").join("runs").join("legacy-alias-run");
    fs::create_dir_all(&run_dir).unwrap();

    let events = r#"{"id":"e1","run_id":"legacy-alias-run","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}
{"id":"e2","run_id":"legacy-alias-run","ts":"2024-01-01T00:00:01Z","schema_version":1,"kind":"run_completed"}
"#;
    fs::write(run_dir.join("event-log.jsonl"), events).unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    cmd.arg("run")
        .arg("show")
        .arg("legacy-alias-run")
        .arg("--kind")
        .arg("run_completed");

    cmd.assert()
        .success()
        .stdout(contains("📋 Run timeline — legacy-alias-run (1 events)"))
        .stdout(contains("Source:"))
        .stdout(contains("event-log.jsonl"))
        .stdout(contains("run_completed"));
}
