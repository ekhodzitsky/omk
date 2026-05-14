use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn test_autopilot_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("autopilot").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("resume"));
}

#[test]
fn test_ralph_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("ralph").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("yolo"));
}

#[test]
fn test_help_smoke() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--help");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("doctor").arg("--help");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("config").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_north_star_demo_exits_non_zero_when_proof_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let fake_kimi = fake_bin.join("kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "--help" ]]; then
  echo "Usage: omk goal"
  exit 0
fi
if [[ "${1:-}" == "setup" ]]; then
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "run" ]]; then
  echo "Goal run finished"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--json" ]]; then
  echo '{"status":"not_ready","phase":"proof","until_ready":true,"state_dir":"/tmp/fake-goal"}'
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "replay" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Goal replay"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "proof" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo "2026-05-14T00:00:00Z INFO fake log before json"
  echo '{"status":"failed_infra","readiness":"infra failed","known_gaps":["mock failure"]}'
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(&fake_kimi, "#!/usr/bin/env bash\nexit 0\n").unwrap();

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&fake_omk).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, perms).unwrap();
        let mut kimi_perms = fs::metadata(&fake_kimi).unwrap().permissions();
        kimi_perms.set_mode(0o755);
        fs::set_permissions(&fake_kimi, kimi_perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let output = std::process::Command::new("bash")
        .arg(script_path())
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "north_star_demo.sh must return non-zero when proof status=failed_infra"
    );
}

#[test]
fn test_north_star_demo_mock_mode_isolates_home_and_xdg() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let log_path = tmp.path().join("fake-omk.log");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "--help" ]]; then
  echo "Usage: omk goal"
  exit 0
fi
if [[ "${1:-}" == "setup" ]]; then
  echo "setup HOME=${HOME:-} XDG_STATE_HOME=${XDG_STATE_HOME:-} XDG_CONFIG_HOME=${XDG_CONFIG_HOME:-} XDG_CACHE_HOME=${XDG_CACHE_HOME:-}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "run" ]]; then
  if [[ -z "${MOCK_KIMI:-}" || ! -x "${MOCK_KIMI}" ]]; then
    echo "missing executable MOCK_KIMI: ${MOCK_KIMI:-}" >&2
    exit 2
  fi
  echo "goal HOME=${HOME:-} XDG_STATE_HOME=${XDG_STATE_HOME:-} XDG_CONFIG_HOME=${XDG_CONFIG_HOME:-} XDG_CACHE_HOME=${XDG_CACHE_HOME:-} MOCK_KIMI=${MOCK_KIMI}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--json" ]]; then
  echo '{"status":"ready","phase":"done","until_ready":true,"state_dir":"/tmp/fake-goal"}'
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "replay" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Goal replay"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "proof" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo '{"status":"ready","readiness":"ready","known_gaps":[]}'
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&fake_omk).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let output = std::process::Command::new("bash")
        .arg(script_path())
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .env("MOCK_KIMI", "1")
        .env("FAKE_OMK_LOG", &log_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "north_star_demo.sh should succeed in MOCK_KIMI=1 mode"
    );

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains("setup HOME=/tmp/omk-north-star-"));
    assert!(log.contains("goal HOME=/tmp/omk-north-star-"));
    assert!(log.contains("XDG_STATE_HOME=/tmp/omk-north-star-"));
    assert!(log.contains("XDG_CONFIG_HOME=/tmp/omk-north-star-"));
    assert!(log.contains("XDG_CACHE_HOME=/tmp/omk-north-star-"));
    assert!(log.contains("MOCK_KIMI=/tmp/omk-north-star-"));
}

#[test]
fn test_north_star_demo_accepts_custom_executable_mock_kimi_path() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let log_path = tmp.path().join("fake-omk.log");
    let custom_mock_kimi = tmp.path().join("custom-mock-kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "--help" ]]; then
  echo "Usage: omk goal"
  exit 0
fi
if [[ "${1:-}" == "setup" ]]; then
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "run" ]]; then
  if [[ -z "${MOCK_KIMI:-}" || ! -x "${MOCK_KIMI}" ]]; then
    echo "missing executable MOCK_KIMI: ${MOCK_KIMI:-}" >&2
    exit 2
  fi
  echo "goal MOCK_KIMI=${MOCK_KIMI}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--json" ]]; then
  echo '{"status":"ready","phase":"done","until_ready":true,"state_dir":"/tmp/fake-goal"}'
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "replay" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Goal replay"
  exit 0
fi
if [[ "${1:-}" == "goal" && "${2:-}" == "proof" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo '{"status":"ready","readiness":"ready","known_gaps":[]}'
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(&custom_mock_kimi, "#!/usr/bin/env bash\nexit 0\n").unwrap();

    #[cfg(unix)]
    {
        let mut omk_perms = fs::metadata(&fake_omk).unwrap().permissions();
        omk_perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, omk_perms).unwrap();

        let mut mock_perms = fs::metadata(&custom_mock_kimi).unwrap().permissions();
        mock_perms.set_mode(0o755);
        fs::set_permissions(&custom_mock_kimi, mock_perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let output = std::process::Command::new("bash")
        .arg(script_path())
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .env("MOCK_KIMI", &custom_mock_kimi)
        .env("FAKE_OMK_LOG", &log_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "north_star_demo.sh should accept an executable custom MOCK_KIMI path"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_mock_line = format!("MOCK_KIMI={}", custom_mock_kimi.display());
    assert!(stdout.contains(&expected_mock_line));

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains(&expected_mock_line));
}

fn script_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh")
}
