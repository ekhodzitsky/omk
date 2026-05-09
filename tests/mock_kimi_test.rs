use assert_cmd::Command;
use predicates::str::contains;
use std::io::Write;
use tempfile::NamedTempFile;

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

    let bin = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| "mock-kimi".to_string());
    let mut child = StdCommand::new(&bin)
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

    let bin = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| "mock-kimi".to_string());
    let mut child = StdCommand::new(&bin)
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
fn test_wire_slow_mode_emits_delayed_event() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as StdCommand, Stdio};
    use std::time::{Duration, Instant};

    let bin = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| "mock-kimi".to_string());
    let mut child = StdCommand::new(&bin)
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

    let bin = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| "mock-kimi".to_string());
    let mut child = StdCommand::new(&bin)
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
    use omk::wire::client::WireClient;
    use omk::wire::protocol::InitializeParams;

    let bin = std::env::var("CARGO_BIN_EXE_mock-kimi").unwrap_or_else(|_| "mock-kimi".to_string());
    let mut client = WireClient::spawn(&bin, None, None, None).unwrap();

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
