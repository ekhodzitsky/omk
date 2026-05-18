use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn test_north_star_demo_rejects_non_executable_mock_kimi_path() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let not_executable_mock = tmp.path().join("not-executable-mock-kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(&not_executable_mock, "not executable\n").unwrap();

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
        .env("MOCK_KIMI", &not_executable_mock)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "north_star_demo.sh must fail when MOCK_KIMI points to a non-executable file"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!(
        "MOCK_KIMI is set but not executable: {}",
        not_executable_mock.display()
    );
    assert!(stdout.contains(&expected));
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn test_north_star_demo_dry_run_does_not_require_kimi_for_setup() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();

    let isolated_script_dir = tmp.path().join("isolated-scripts");
    fs::create_dir_all(&isolated_script_dir).unwrap();
    let isolated_script_path = isolated_script_dir.join("north_star_demo.sh");
    fs::copy(script_path(), &isolated_script_path).unwrap();

    #[cfg(unix)]
    {
        let mut omk_perms = fs::metadata(&fake_omk).unwrap().permissions();
        omk_perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, omk_perms).unwrap();

        let mut script_perms = fs::metadata(&isolated_script_path).unwrap().permissions();
        script_perms.set_mode(0o755);
        fs::set_permissions(&isolated_script_path, script_perms).unwrap();
    }

    let output = std::process::Command::new("/bin/bash")
        .arg(&isolated_script_path)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", fake_bin.display()),
        )
        .env("NORTH_STAR_DRY_RUN", "1")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "fake omk lacks goal support, so the demo should fail at the goal runtime gate"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running in MOCK mode (no real Kimi needed)"));
    assert!(stdout.contains("omk goal runtime is unavailable"));
    assert!(!stdout.contains("Neither 'kimi' nor 'mock-kimi' found."));
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn test_north_star_demo_dry_run_ignores_unusable_real_kimi_path() {
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
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(
        &fake_kimi,
        "#!/usr/bin/env bash\necho 'kimi version 1.0.0'\nexit 0\n",
    )
    .unwrap();

    #[cfg(unix)]
    {
        let mut omk_perms = fs::metadata(&fake_omk).unwrap().permissions();
        omk_perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, omk_perms).unwrap();

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
        "north_star_demo.sh must fail at the goal runtime gate with this fake omk"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running in MOCK mode (no real Kimi needed)"));
    assert!(stdout.contains("omk goal runtime is unavailable"));
    assert!(!stdout.contains("kimi auth required"));
}

fn script_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh")
}
