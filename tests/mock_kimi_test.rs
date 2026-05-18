use assert_cmd::Command;
use predicates::str::contains;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[path = "fixtures/team_demo_fixture.rs"]
mod team_demo_fixture;

fn mock_kimi_path() -> PathBuf {
    assert_cmd::cargo::cargo_bin("mock-kimi")
}

fn mock_kimi_path_string() -> String {
    mock_kimi_path().to_string_lossy().into_owned()
}

fn mock_kimi() -> Command {
    Command::cargo_bin("mock-kimi").unwrap()
}

#[test]
fn test_version() {
    mock_kimi()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("kimi version 0.1.0-mock"));
}

#[test]
fn test_help() {
    mock_kimi()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("mock-kimi"))
        .stdout(contains("--malformed"));
}

#[test]
fn test_prompt_normal() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "Implement a hash function in Rust").unwrap();

    mock_kimi()
        .arg("-p")
        .arg(file.path())
        .assert()
        .success()
        .stdout(contains("\"status\":\"success\""))
        .stdout(contains("\"mock\":true"))
        .stdout(contains("Implement a hash function"));
}

#[test]
fn test_prompt_test_keyword() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "run the test suite").unwrap();

    mock_kimi()
        .arg("-p")
        .arg(file.path())
        .assert()
        .success()
        .stdout(contains("\"status\":\"success\""))
        .stdout(contains("I see you want to run tests."));
}

#[test]
fn test_prompt_error_keyword() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "this should error").unwrap();

    mock_kimi()
        .arg("-p")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(contains("\"status\":\"error\""))
        .stderr(contains("Mock error triggered"));
}

#[test]
fn test_prompt_fail_keyword() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "this will fail").unwrap();

    mock_kimi()
        .arg("-p")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(contains("\"status\":\"error\""));
}

#[test]
fn test_prompt_missing_file() {
    mock_kimi()
        .arg("-p")
        .arg("/nonexistent/path/prompt.txt")
        .assert()
        .failure()
        .stderr(contains("\"status\":\"error\""));
}

#[test]
fn test_wire_stall_mode() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as StdCommand, Stdio};
    use std::thread;
    use std::time::Duration;

    let mut child = StdCommand::new(mock_kimi_path())
        .arg("--wire")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mock-kimi");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    // Send initialize request
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"initialize","id":"init-1","params":{{}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    // Send prompt that triggers stall via keyword
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"prompt","id":"prompt-1","params":{{"user_input":"please stall for me"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    // Read until we see turn_begin
    let mut saw_turn_begin = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if line.contains("turn_begin") {
            saw_turn_begin = true;
            break;
        }
    }
    assert!(
        saw_turn_begin,
        "Expected turn_begin event before entering stall"
    );

    // Give it a moment to enter the stall loop
    thread::sleep(Duration::from_millis(200));

    // Kill the process
    child.kill().expect("Failed to kill child");
    let status = child.wait().expect("Failed to wait on child");

    // Should not have exited cleanly (was killed)
    assert!(
        !status.success(),
        "Expected process to be killed, not exit cleanly"
    );
}

#[test]
fn test_wire_stall_mode_with_flag() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as StdCommand, Stdio};
    use std::thread;
    use std::time::Duration;

    let mut child = StdCommand::new(mock_kimi_path())
        .arg("--wire")
        .arg("--stall")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mock-kimi");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    // Send initialize request
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"initialize","id":"init-1","params":{{}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    // Send a normal prompt (stall is triggered by --stall flag)
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"prompt","id":"prompt-1","params":{{"user_input":"hello world"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    // Read until we see turn_begin
    let mut saw_turn_begin = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if line.contains("turn_begin") {
            saw_turn_begin = true;
            break;
        }
    }
    assert!(
        saw_turn_begin,
        "Expected turn_begin event before entering stall"
    );

    // Wait briefly
    thread::sleep(Duration::from_millis(200));

    // Kill the process
    child.kill().expect("Failed to kill child");
    let status = child.wait().expect("Failed to wait on child");

    assert!(
        !status.success(),
        "Expected process to be killed, not exit cleanly"
    );
}

#[test]
fn test_wire_crash_after_turn_begin_mode() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as StdCommand, Stdio};

    let mut child = StdCommand::new(mock_kimi_path())
        .arg("--wire")
        .arg("--crash-after-turn-begin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mock-kimi");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"initialize","id":"init-1","params":{{}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"prompt","id":"prompt-1","params":{{"user_input":"please crash-after-turn-begin now"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    let mut saw_turn_begin = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if line.contains("turn_begin") {
            saw_turn_begin = true;
            break;
        }
    }
    assert!(
        saw_turn_begin,
        "Expected turn_begin event before crash in crash-after-turn-begin mode"
    );

    let status = child.wait().expect("Failed to wait on child");
    assert!(
        !status.success(),
        "Expected crash-after-turn-begin mode to exit with failure"
    );
}

#[test]
fn test_wire_slow_mode_emits_delayed_event() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as StdCommand, Stdio};
    use std::time::{Duration, Instant};

    let mut child = StdCommand::new(mock_kimi_path())
        .arg("--wire")
        .arg("--slow")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mock-kimi");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"initialize","id":"init-1","params":{{}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("\"protocol_version\":\"1.9\""));

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"prompt","id":"prompt-1","params":{{"user_input":"hello world"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("\"status\":\"ok\""));

    let started = Instant::now();
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("turn_begin"));
    assert!(started.elapsed() >= Duration::from_millis(800));

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_wire_malformed_mode_emits_invalid_json() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as StdCommand, Stdio};

    let mut child = StdCommand::new(mock_kimi_path())
        .arg("--wire")
        .arg("--malformed")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn mock-kimi");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"initialize","id":"init-1","params":{{}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "{ this is not valid json");

    let status = child.wait().expect("Failed to wait on child");
    assert!(status.success());
}

#[tokio::test]
async fn test_wire_control_methods() {
    use omk::wire::client::{ProcessWireClient, WireClient};
    use omk::wire::protocol::InitializeParams;

    let bin = mock_kimi_path_string();
    let mut client = ProcessWireClient::spawn(&bin, None, None, None)
        .await
        .unwrap();

    let init = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();
    assert_eq!(init.protocol_version, "1.9");

    let replay = client.replay().await.unwrap();
    assert_eq!(replay.status, "finished");
    assert_eq!(replay.events.unwrap().len(), 0);
    assert_eq!(replay.requests.unwrap().len(), 0);

    let steer = client.steer("keep it concise").await.unwrap();
    assert_eq!(steer.status, "steered");

    let plan = client.set_plan_mode(true).await.unwrap();
    assert_eq!(plan.plan_mode, Some(true));

    client.cancel().await.unwrap();
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_team_demo_fixture_scripted_outcomes_are_stable() {
    use team_demo_fixture::{build_stable_demo_output, read_demo_output, TeamDemoFixture};

    let mut fixture = TeamDemoFixture::new().await;
    let result = fixture.run().await;

    assert!(matches!(
        result.proof.status,
        omk::runtime::proof::ProofStatus::Failed
    ));
    assert!(result
        .proof
        .gates
        .iter()
        .any(|gate| gate.name == "verification"
            && matches!(gate.status, omk::runtime::proof::GateStatus::Failed)));
    assert!(result
        .proof
        .failures
        .iter()
        .any(|failure| failure.description == "worker stalled"));
    assert!(matches!(
        result.worker_results.get("worker-0"),
        Some(Some(summary)) if summary.starts_with("Success:")
    ));
    assert!(matches!(
        result.worker_results.get("worker-1"),
        Some(Some(summary)) if summary.starts_with("Failed:")
    ));
    assert!(matches!(result.worker_results.get("worker-2"), Some(None)));

    let rendered = build_stable_demo_output(&result.proof, &result.worker_results);
    let from_file = read_demo_output(&fixture.state_dir);
    assert_eq!(result.stable_demo_output, rendered);
    assert_eq!(from_file, rendered);
    assert!(rendered.contains("outcomes=success,failed_verification,stalled_worker"));
    assert!(rendered.contains("workers=worker-0:success,worker-1:failed,worker-2:stalled"));
}

#[tokio::test]
async fn test_wire_worker_adapter_cancellation_stops_idle_worker() {
    use omk::runtime::events::{EventWriter, RunId};
    use omk::runtime::wire_worker::WireWorkerAdapter;
    use omk::runtime::worker::WorkerSpec;
    use tempfile::TempDir;
    use tokio::time::Duration;
    use tokio_util::sync::CancellationToken;

    let tmp = TempDir::new().unwrap();
    let worker_dir = tmp.path().join("worker-cancel");
    let project_dir = tmp.path().join("project");
    tokio::fs::create_dir_all(&worker_dir).await.unwrap();
    tokio::fs::create_dir_all(&project_dir).await.unwrap();

    let spec = WorkerSpec {
        name: "worker-cancel".to_string(),
        role: "coder".to_string(),
        inbox: worker_dir.join("inbox.jsonl"),
        outbox: worker_dir.join("outbox.jsonl"),
        heartbeat: worker_dir.join("heartbeat.json"),
        project_dir: Some(project_dir),
        external_tools: None,
        approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
        approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
    };
    spec.save().await.unwrap();

    let event_writer = EventWriter::new(tmp.path().join("events.jsonl"));
    let run_id = RunId("run-wire-cancel".to_string());
    let cancel = CancellationToken::new();
    let adapter =
        WireWorkerAdapter::new_with_cancel(spec.clone(), run_id, event_writer, cancel.clone());
    let handle = adapter.spawn();

    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("adapter should stop after cancellation")
        .expect("adapter task should not panic");

    let heartbeat = tokio::fs::read_to_string(&spec.heartbeat).await.unwrap();
    let heartbeat: serde_json::Value = serde_json::from_str(&heartbeat).unwrap();
    assert_eq!(heartbeat["status"], "stopped");
}

#[tokio::test]
async fn test_wire_worker_adapter_times_out_stalled_turn_and_writes_failed_result() {
    use omk::runtime::events::{EventWriter, RunId};
    use omk::runtime::wire_worker::WireWorkerAdapter;
    use omk::runtime::worker::{ResultStatus, WorkerSpec, WorkerTask};
    use tempfile::TempDir;
    use tokio::time::{Duration, Instant};

    let tmp = TempDir::new().unwrap();
    let worker_dir = tmp.path().join("worker-timeout");
    let project_dir = tmp.path().join("project");
    tokio::fs::create_dir_all(&worker_dir).await.unwrap();
    tokio::fs::create_dir_all(&project_dir).await.unwrap();

    let spec = WorkerSpec {
        name: "worker-timeout".to_string(),
        role: "coder".to_string(),
        inbox: worker_dir.join("inbox.jsonl"),
        outbox: worker_dir.join("outbox.jsonl"),
        heartbeat: worker_dir.join("heartbeat.json"),
        project_dir: Some(project_dir),
        external_tools: None,
        approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
        approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
    };
    spec.save().await.unwrap();

    let events_path = tmp.path().join("events.jsonl");
    let event_writer = EventWriter::new(&events_path);
    let run_id = RunId("run-wire-timeout".to_string());

    let bin = mock_kimi_path();
    let prev_mock = std::env::var("MOCK_KIMI").ok();
    let prev_timeout_ms = std::env::var("OMK_WIRE_TURN_TIMEOUT_MS").ok();
    std::env::set_var("MOCK_KIMI", bin.as_os_str());
    std::env::set_var("OMK_WIRE_TURN_TIMEOUT_MS", "300");

    let adapter = WireWorkerAdapter::new(spec.clone(), run_id, event_writer);
    let handle = adapter.spawn();

    spec.send_task(&WorkerTask {
        id: "task-timeout".to_string(),
        task: "please stall forever".to_string(),
        acceptance_criteria: vec![],
        context: None,
        budget_secs: None,
    })
    .await
    .unwrap();

    let started = Instant::now();
    let mut found = None;
    while started.elapsed() < Duration::from_secs(12) {
        let results = spec.read_results().await.unwrap();
        if let Some(first) = results.first() {
            found = Some(first.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    handle.abort();
    let _ = handle.await;

    match prev_mock {
        Some(v) => std::env::set_var("MOCK_KIMI", v),
        None => std::env::remove_var("MOCK_KIMI"),
    }
    match prev_timeout_ms {
        Some(v) => std::env::set_var("OMK_WIRE_TURN_TIMEOUT_MS", v),
        None => std::env::remove_var("OMK_WIRE_TURN_TIMEOUT_MS"),
    }

    let result = found.expect("expected failed worker result after wire turn timeout");
    assert!(matches!(result.status, ResultStatus::Failed));
    let summary = result.summary.to_lowercase();
    assert!(summary.contains("timeout") || summary.contains("timed out"));

    let events = tokio::fs::read_to_string(events_path).await.unwrap();
    assert!(events.contains("\"task_failed\""));
    let events_lower = events.to_lowercase();
    assert!(events_lower.contains("timeout") || events_lower.contains("timed out"));
}

#[tokio::test]
async fn test_wire_worker_adapter_enforces_task_budget_timeout() {
    use omk::runtime::events::{EventWriter, RunId};
    use omk::runtime::wire_worker::WireWorkerAdapter;
    use omk::runtime::worker::{ResultStatus, WorkerSpec, WorkerTask};
    use tempfile::TempDir;
    use tokio::time::{Duration, Instant};

    let tmp = TempDir::new().unwrap();
    let worker_dir = tmp.path().join("worker-budget-timeout");
    let project_dir = tmp.path().join("project");
    tokio::fs::create_dir_all(&worker_dir).await.unwrap();
    tokio::fs::create_dir_all(&project_dir).await.unwrap();

    let spec = WorkerSpec {
        name: "worker-budget-timeout".to_string(),
        role: "coder".to_string(),
        inbox: worker_dir.join("inbox.jsonl"),
        outbox: worker_dir.join("outbox.jsonl"),
        heartbeat: worker_dir.join("heartbeat.json"),
        project_dir: Some(project_dir),
        external_tools: None,
        approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
        approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
    };
    spec.save().await.unwrap();

    let events_path = tmp.path().join("events.jsonl");
    let event_writer = EventWriter::new(&events_path);
    let run_id = RunId("run-wire-budget-timeout".to_string());

    let bin = mock_kimi_path();
    let prev_mock = std::env::var("MOCK_KIMI").ok();
    let prev_turn_timeout_ms = std::env::var("OMK_WIRE_TURN_TIMEOUT_MS").ok();
    let prev_poll_ms = std::env::var("OMK_WIRE_WORKER_POLL_INTERVAL_MS").ok();
    std::env::set_var("MOCK_KIMI", bin.as_os_str());
    std::env::set_var("OMK_WIRE_TURN_TIMEOUT_MS", "10000");
    std::env::set_var("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50");

    let adapter = WireWorkerAdapter::new(spec.clone(), run_id, event_writer);
    let handle = adapter.spawn();

    spec.send_task(&WorkerTask {
        id: "task-budget-timeout".to_string(),
        task: "please stall forever".to_string(),
        acceptance_criteria: vec![],
        context: None,
        budget_secs: Some(1),
    })
    .await
    .unwrap();

    let has_timeout_event = |events: &str| {
        events.lines().any(|line| {
            let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
                return false;
            };
            let Some(payload) = event.get("payload") else {
                return false;
            };
            payload.get("type").and_then(|v| v.as_str()) == Some("task_budget_timeout")
                && payload.get("task_id").and_then(|v| v.as_str()) == Some("task-budget-timeout")
                && payload.get("timeout_secs").and_then(|v| v.as_u64()) == Some(1)
        })
    };

    let started = Instant::now();
    let mut found = None;
    let mut found_timeout_event = false;
    let mut latest_events = String::new();
    while started.elapsed() < Duration::from_secs(5) {
        if found.is_none() {
            let results = spec.read_results().await.unwrap();
            if let Some(first) = results.first() {
                found = Some(first.clone());
            }
        }
        if !found_timeout_event {
            latest_events = tokio::fs::read_to_string(&events_path)
                .await
                .unwrap_or_default();
            found_timeout_event = has_timeout_event(&latest_events);
        }
        if found.is_some() && found_timeout_event {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    handle.abort();
    let _ = handle.await;

    match prev_mock {
        Some(v) => std::env::set_var("MOCK_KIMI", v),
        None => std::env::remove_var("MOCK_KIMI"),
    }
    match prev_turn_timeout_ms {
        Some(v) => std::env::set_var("OMK_WIRE_TURN_TIMEOUT_MS", v),
        None => std::env::remove_var("OMK_WIRE_TURN_TIMEOUT_MS"),
    }
    match prev_poll_ms {
        Some(v) => std::env::set_var("OMK_WIRE_WORKER_POLL_INTERVAL_MS", v),
        None => std::env::remove_var("OMK_WIRE_WORKER_POLL_INTERVAL_MS"),
    }

    let result = found.expect("expected failed worker result after task budget timeout");
    assert!(matches!(result.status, ResultStatus::Failed));
    assert!(result.summary.contains("task budget timed out after 1s"));
    assert!(
        found_timeout_event,
        "expected structured task_budget_timeout event with timeout_secs=1, got:\n{latest_events}"
    );
}

#[tokio::test]
async fn test_wire_worker_adapter_handles_mid_task_crash_after_turn_begin() {
    use omk::runtime::events::{EventWriter, RunId};
    use omk::runtime::wire_worker::WireWorkerAdapter;
    use omk::runtime::worker::{ResultStatus, WorkerSpec, WorkerTask};
    use tempfile::TempDir;
    use tokio::time::{Duration, Instant};

    let tmp = TempDir::new().unwrap();
    let worker_dir = tmp.path().join("worker-crash-mid-task");
    let project_dir = tmp.path().join("project");
    tokio::fs::create_dir_all(&worker_dir).await.unwrap();
    tokio::fs::create_dir_all(&project_dir).await.unwrap();

    let spec = WorkerSpec {
        name: "worker-crash-mid-task".to_string(),
        role: "coder".to_string(),
        inbox: worker_dir.join("inbox.jsonl"),
        outbox: worker_dir.join("outbox.jsonl"),
        heartbeat: worker_dir.join("heartbeat.json"),
        project_dir: Some(project_dir),
        external_tools: None,
        approval_policy: omk::runtime::wire_worker::ApprovalPolicy::default(),
        approval_timeout_secs: omk::runtime::worker::default_approval_timeout_secs(),
    };
    spec.save().await.unwrap();

    let events_path = tmp.path().join("events.jsonl");
    let event_writer = EventWriter::new(&events_path);
    let run_id = RunId("run-wire-crash-mid-task".to_string());

    let bin = mock_kimi_path();
    let prev_mock = std::env::var("MOCK_KIMI").ok();
    std::env::set_var("MOCK_KIMI", bin.as_os_str());

    let adapter = WireWorkerAdapter::new(spec.clone(), run_id, event_writer);
    let handle = adapter.spawn();

    spec.send_task(&WorkerTask {
        id: "task-crash-mid-task".to_string(),
        task: "please crash-after-turn-begin right after turn start".to_string(),
        acceptance_criteria: vec![],
        context: None,
        budget_secs: None,
    })
    .await
    .unwrap();

    let started = Instant::now();
    let mut found = None;
    while started.elapsed() < Duration::from_secs(12) {
        let results = spec.read_results().await.unwrap();
        if let Some(first) = results.first() {
            found = Some(first.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    handle.abort();
    let _ = handle.await;

    match prev_mock {
        Some(v) => std::env::set_var("MOCK_KIMI", v),
        None => std::env::remove_var("MOCK_KIMI"),
    }

    let result = found.expect("expected failed worker result after mid-task crash");
    assert!(matches!(result.status, ResultStatus::Failed));
    let summary = result.summary.to_lowercase();
    assert!(
        summary.contains("error")
            || summary.contains("eof")
            || summary.contains("closed")
            || summary.contains("broken pipe")
            || summary.contains("aborted")
            || summary.contains("failed"),
        "unexpected crash summary: {}",
        result.summary
    );

    let events = tokio::fs::read_to_string(events_path).await.unwrap();
    assert!(events.contains("\"task_failed\""));
    assert!(events.contains("task-crash-mid-task"));
}
