use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn test_ultrawork_cli_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.args(["ultrawork", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("Parallel burst execution"));
}

#[test]
fn test_ultrawork_alias_uw() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.args(["uw", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("Parallel burst execution"));
}

#[test]
fn test_ultrawork_requires_tasks() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.args(["ultrawork"]);
    cmd.assert().failure().stderr(contains("No tasks provided"));
}
