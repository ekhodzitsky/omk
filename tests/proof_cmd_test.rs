use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::process::Command as StdCommand;

fn write_minimal_events(dir: &std::path::Path, run_id: &str) {
    let events = format!(
        r#"{{"id":"e1","run_id":"{run_id}","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}}
{{"id":"e2","run_id":"{run_id}","ts":"2024-01-01T00:01:00Z","schema_version":1,"kind":"run_completed"}}
"#
    );
    fs::write(dir.join("events.jsonl"), events).unwrap();
}

fn setup_env(tmp: &tempfile::TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let home = tmp.path().join("home");
    let xdg_state = tmp.path().join("xdg_state");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&xdg_state).unwrap();
    (home, xdg_state)
}

#[test]
fn test_proof_show_latest_resolves_chronologically() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, xdg_state) = setup_env(&tmp);

    let runs_dir = xdg_state.join("omk").join("runs");
    fs::create_dir_all(&runs_dir).unwrap();

    let run_aaa = runs_dir.join("run-aaa");
    let run_bbb = runs_dir.join("run-bbb");
    let run_ccc = runs_dir.join("run-ccc");

    fs::create_dir(&run_aaa).unwrap();
    fs::create_dir(&run_bbb).unwrap();
    fs::create_dir(&run_ccc).unwrap();

    write_minimal_events(&run_aaa, "run-aaa");
    write_minimal_events(&run_bbb, "run-bbb");
    write_minimal_events(&run_ccc, "run-ccc");

    // run-bbb is newest, run-ccc is middle, run-aaa is oldest.
    StdCommand::new("touch")
        .args(["-t", "202401010000", run_aaa.to_str().unwrap()])
        .status()
        .unwrap();
    StdCommand::new("touch")
        .args(["-t", "202401010002", run_bbb.to_str().unwrap()])
        .status()
        .unwrap();
    StdCommand::new("touch")
        .args(["-t", "202401010001", run_ccc.to_str().unwrap()])
        .status()
        .unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.env("HOME", &home)
        .env("XDG_STATE_HOME", &xdg_state)
        .arg("proof")
        .arg("show")
        .arg("latest");

    cmd.assert()
        .success()
        .stdout(contains("Proof Report for run-bbb"));
}

#[test]
fn test_proof_show_latest_falls_back_to_name() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, xdg_state) = setup_env(&tmp);

    let runs_dir = xdg_state.join("omk").join("runs");
    fs::create_dir_all(&runs_dir).unwrap();

    let run_aaa = runs_dir.join("run-aaa");
    let run_bbb = runs_dir.join("run-bbb");
    let run_ccc = runs_dir.join("run-ccc");

    fs::create_dir(&run_aaa).unwrap();
    fs::create_dir(&run_bbb).unwrap();
    fs::create_dir(&run_ccc).unwrap();

    write_minimal_events(&run_aaa, "run-aaa");
    write_minimal_events(&run_bbb, "run-bbb");
    write_minimal_events(&run_ccc, "run-ccc");

    // Identical mtimes — force fallback to name sort.
    for dir in [&run_aaa, &run_bbb, &run_ccc] {
        StdCommand::new("touch")
            .args(["-t", "202401010000", dir.to_str().unwrap()])
            .status()
            .unwrap();
    }

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.env("HOME", &home)
        .env("XDG_STATE_HOME", &xdg_state)
        .arg("proof")
        .arg("show")
        .arg("latest");

    // Reverse alphabetical fallback: run-ccc > run-bbb > run-aaa.
    cmd.assert()
        .success()
        .stdout(contains("Proof Report for run-ccc"));
}

#[test]
fn test_proof_show_text_output() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, xdg_state) = setup_env(&tmp);

    let runs_dir = xdg_state.join("omk").join("runs");
    fs::create_dir_all(&runs_dir).unwrap();

    let run_dir = runs_dir.join("text-run");
    fs::create_dir(&run_dir).unwrap();

    // Richer events: file change + gate passed.
    let events = r#"{"id":"e1","run_id":"text-run","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}
{"id":"e2","run_id":"text-run","ts":"2024-01-01T00:00:01Z","schema_version":1,"kind":"worker_started","actor":"w1"}
{"id":"e3","run_id":"text-run","ts":"2024-01-01T00:00:02Z","schema_version":1,"kind":"file_changed","payload":{"path":"src/main.rs","operation":"modified"}}
{"id":"e4","run_id":"text-run","ts":"2024-01-01T00:00:03Z","schema_version":1,"kind":"gate_passed","payload":{"gate_id":"g1","name":"fmt","required":true}}
{"id":"e5","run_id":"text-run","ts":"2024-01-01T00:01:00Z","schema_version":1,"kind":"run_completed"}
"#;
    fs::write(run_dir.join("events.jsonl"), events).unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.env("HOME", &home)
        .env("XDG_STATE_HOME", &xdg_state)
        .arg("proof")
        .arg("show")
        .arg("--format")
        .arg("text")
        .arg("text-run");

    cmd.assert()
        .success()
        .stdout(contains("Proof Report for text-run"))
        .stdout(contains("Status:      ready"))
        .stdout(contains("Readiness:   ready_for_handoff"))
        .stdout(contains("Verdict:"))
        .stdout(contains("Changed files (1):"))
        .stdout(contains("Gates (1):"))
        .stdout(contains("Failures (0):"))
        .stdout(contains("Retries (0):"))
        .stdout(contains("Known gaps (0):"))
        .stdout(contains("Readiness verdict: ready_for_handoff."));
}

#[test]
fn test_proof_show_md_output() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, xdg_state) = setup_env(&tmp);

    let runs_dir = xdg_state.join("omk").join("runs");
    fs::create_dir_all(&runs_dir).unwrap();

    let run_dir = runs_dir.join("md-run");
    fs::create_dir(&run_dir).unwrap();

    let events = r#"{"id":"e1","run_id":"md-run","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}
{"id":"e2","run_id":"md-run","ts":"2024-01-01T00:00:01Z","schema_version":1,"kind":"worker_started","actor":"w1"}
{"id":"e3","run_id":"md-run","ts":"2024-01-01T00:00:02Z","schema_version":1,"kind":"file_changed","payload":{"path":"src/main.rs","operation":"modified"}}
{"id":"e4","run_id":"md-run","ts":"2024-01-01T00:00:03Z","schema_version":1,"kind":"gate_passed","payload":{"gate_id":"g1","name":"fmt","required":true}}
{"id":"e5","run_id":"md-run","ts":"2024-01-01T00:01:00Z","schema_version":1,"kind":"run_completed"}
"#;
    fs::write(run_dir.join("events.jsonl"), events).unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.env("HOME", &home)
        .env("XDG_STATE_HOME", &xdg_state)
        .arg("proof")
        .arg("show")
        .arg("--format")
        .arg("md")
        .arg("md-run");

    cmd.assert()
        .success()
        .stdout(contains("# Proof Report for md-run"))
        .stdout(contains("**Status:**"))
        .stdout(contains("**Readiness:** ready_for_handoff"))
        .stdout(contains("## Verdict"))
        .stdout(contains("## Changed Files"))
        .stdout(contains("## Gates"))
        .stdout(contains("Readiness verdict: `ready_for_handoff`."));
}

#[test]
fn test_proof_show_json_output_includes_failures_retries_and_gaps() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, xdg_state) = setup_env(&tmp);

    let runs_dir = xdg_state.join("omk").join("runs");
    fs::create_dir_all(&runs_dir).unwrap();

    let run_dir = runs_dir.join("json-run");
    fs::create_dir(&run_dir).unwrap();

    let events = r#"{"id":"e1","run_id":"json-run","ts":"2024-01-01T00:00:00Z","schema_version":1,"kind":"run_started"}
{"id":"e2","run_id":"json-run","ts":"2024-01-01T00:00:01Z","schema_version":1,"kind":"file_changed","payload":{"path":"src/lib.rs","operation":"modified"}}
{"id":"e3","run_id":"json-run","ts":"2024-01-01T00:00:02Z","schema_version":1,"kind":"gate_failed","payload":{"gate_id":"g1","name":"test","required":true}}
{"id":"e4","run_id":"json-run","ts":"2024-01-01T00:00:03Z","schema_version":1,"kind":"retry_scheduled","payload":{"task_id":"task-1","attempt":2,"reason":"flake"}}
{"id":"e5","run_id":"json-run","ts":"2024-01-01T00:00:04Z","schema_version":1,"kind":"run_failed","payload":{"reason":"gate failure"}}
"#;
    fs::write(run_dir.join("events.jsonl"), events).unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.env("HOME", &home)
        .env("XDG_STATE_HOME", &xdg_state)
        .arg("proof")
        .arg("show")
        .arg("--format")
        .arg("json")
        .arg("json-run");

    cmd.assert()
        .success()
        .stdout(contains("\"status\": \"failed\""))
        .stdout(contains("\"changed_files\""))
        .stdout(contains("\"gates\""))
        .stdout(contains("\"failures\""))
        .stdout(contains("\"retries\""))
        .stdout(contains("\"known_gaps\""))
        .stdout(contains("\"task_id\": \"task-1\""))
        .stdout(contains("\"reason\": \"flake\""));
}

#[test]
fn test_proof_path_naming() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, xdg_state) = setup_env(&tmp);

    let runs_dir = xdg_state.join("omk").join("runs");
    fs::create_dir_all(&runs_dir).unwrap();

    let run_dir = runs_dir.join("path-run");
    fs::create_dir(&run_dir).unwrap();

    write_minimal_events(&run_dir, "path-run");

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.env("HOME", &home)
        .env("XDG_STATE_HOME", &xdg_state)
        .arg("proof")
        .arg("show")
        .arg("path-run");

    cmd.assert().success();

    // proof.json should be written alongside events.jsonl.
    let proof_json = run_dir.join("proof.json");
    assert!(
        proof_json.exists(),
        "proof.json should exist in run directory"
    );
}
