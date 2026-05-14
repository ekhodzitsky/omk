use std::fs;
use std::path::{Path, PathBuf};

mod runtime_goal {
    pub(crate) mod state {
        pub(crate) fn normalize_goal(goal: &str) -> String {
            goal.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }

    pub(crate) mod oracle {
        #![allow(dead_code)]
        include!("../src/runtime/goal/oracle.rs");
    }
}

use runtime_goal::oracle::rewrite::{
    compare_rewrite_oracle, compare_rewrite_oracle_with_intentional_incompatibilities,
    run_rewrite_oracle_command, write_rewrite_oracle_observation_fixture, RewriteOracleComparison,
    RewriteOracleField, RewriteOracleIntentionalIncompatibility, RewriteOracleObservation,
};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn rewrite_fixture(name: &str) -> RewriteOracleObservation {
    let root = fixture_path(name);
    RewriteOracleObservation {
        stdout: fs::read_to_string(root.join("stdout.txt")).expect("fixture stdout"),
        stderr: fs::read_to_string(root.join("stderr.txt")).expect("fixture stderr"),
        exit_code: fs::read_to_string(root.join("exit_code.txt"))
            .expect("fixture exit code")
            .trim()
            .parse()
            .expect("numeric exit code"),
        file_artifacts: read_artifacts(&root.join("files")),
    }
}

fn read_artifacts(root: &Path) -> Vec<(String, String)> {
    let mut artifacts = Vec::new();
    collect_artifacts(root, root, &mut artifacts);
    artifacts.sort_by(|left, right| left.0.cmp(&right.0));
    artifacts
}

fn collect_artifacts(root: &Path, dir: &Path, artifacts: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("artifact dir") {
        let path = entry.expect("artifact entry").path();
        if path.is_dir() {
            collect_artifacts(root, &path, artifacts);
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("artifact relative path")
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let contents = fs::read_to_string(&path).expect("artifact contents");
        artifacts.push((relative, contents));
    }
}

fn mismatch_summary(comparison: &RewriteOracleComparison) -> Vec<(RewriteOracleField, &str, &str)> {
    comparison
        .mismatches
        .iter()
        .map(|mismatch| {
            (
                mismatch.field.clone(),
                mismatch.expected.as_str(),
                mismatch.actual.as_str(),
            )
        })
        .collect()
}

#[test]
fn rewrite_oracle_accepts_matching_command_and_file_artifacts() {
    let source_fixture = fixture_path("rewrite_tiny_source_project");
    assert_eq!(
        fs::read_to_string(source_fixture.join("src/message.txt")).expect("source fixture"),
        "old api says hello\n"
    );

    let expected = rewrite_fixture("rewrite_expected_observation");
    let actual = rewrite_fixture("rewrite_actual_matching_observation");

    let comparison = compare_rewrite_oracle(&expected, &actual);

    assert!(comparison.compatible);
    assert!(comparison.mismatches.is_empty());
}

#[test]
fn python_to_rust_fixture_demo_matches_rewrite_oracle_observations() {
    let demo_root = fixture_path("rewrite_python_to_rust_demo");
    let python_source =
        fs::read_to_string(demo_root.join("python/hello_cli.py")).expect("python source fixture");
    let rust_source =
        fs::read_to_string(demo_root.join("rust/src/main.rs")).expect("rust source fixture");

    assert!(python_source.contains("argparse"));
    assert!(rust_source.contains("std::env::args"));

    let expected = rewrite_fixture("rewrite_python_to_rust_demo/python_observation");
    let actual = rewrite_fixture("rewrite_python_to_rust_demo/rust_observation");
    let comparison = compare_rewrite_oracle(&expected, &actual);

    assert!(comparison.compatible);
    assert!(comparison.mismatches.is_empty());
}

#[test]
fn rewrite_oracle_reports_output_exit_and_file_mismatches() {
    let expected = rewrite_fixture("rewrite_expected_observation");
    let actual = rewrite_fixture("rewrite_actual_mismatched_observation");

    let comparison = compare_rewrite_oracle(&expected, &actual);

    assert!(!comparison.compatible);
    assert_eq!(
        mismatch_summary(&comparison),
        vec![
            (
                RewriteOracleField::Stdout,
                "rewrote src/message.txt\n",
                "rewrote src/other.txt\n"
            ),
            (
                RewriteOracleField::Stderr,
                "\n",
                "warning: partial rewrite\n"
            ),
            (RewriteOracleField::ExitCode, "0", "1"),
            (
                RewriteOracleField::FileArtifact {
                    path: "src/message.txt".to_string(),
                },
                "new api says hello\n",
                "new api says goodbye\n"
            ),
        ]
    );
}

#[test]
fn rewrite_oracle_tracks_intentional_incompatibilities() {
    let expected = rewrite_fixture("rewrite_expected_observation");
    let actual = rewrite_fixture("rewrite_actual_mismatched_observation");

    let comparison = compare_rewrite_oracle_with_intentional_incompatibilities(
        &expected,
        &actual,
        &[
            intentional(
                RewriteOracleField::Stdout,
                "new command names output target",
            ),
            intentional(
                RewriteOracleField::Stderr,
                "warning is accepted during migration",
            ),
            intentional(
                RewriteOracleField::ExitCode,
                "nonzero exit accepted until cutover",
            ),
            intentional(
                RewriteOracleField::FileArtifact {
                    path: "src/message.txt".to_string(),
                },
                "message copy intentionally changes",
            ),
        ],
    );

    assert!(comparison.compatible);
    assert!(comparison.mismatches.is_empty());
    assert_eq!(comparison.intentional_incompatibilities.len(), 4);
}

#[tokio::test]
async fn rewrite_oracle_runner_captures_command_and_file_observation() {
    let temp = tempfile::tempdir().expect("temp project");

    let observation = run_rewrite_oracle_command(
        temp.path(),
        "/bin/sh",
        &[
            "-c",
            "printf 'hello stdout\n'; printf 'warn stderr\n' >&2; mkdir -p out; printf 'artifact\n' > out/result.txt; exit 7",
        ],
        &["out/result.txt"],
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("runner should observe command");

    assert_eq!(observation.stdout, "hello stdout\n");
    assert_eq!(observation.stderr, "warn stderr\n");
    assert_eq!(observation.exit_code, 7);
    assert_eq!(
        observation.file_artifacts,
        vec![("out/result.txt".to_string(), "artifact\n".to_string())]
    );
}

#[tokio::test]
async fn rewrite_oracle_writes_golden_observation_fixture() {
    let temp = tempfile::tempdir().expect("temp fixture");
    let observation = RewriteOracleObservation {
        stdout: "stdout\n".to_string(),
        stderr: "stderr\n".to_string(),
        exit_code: 3,
        file_artifacts: vec![("src/message.txt".to_string(), "hello\n".to_string())],
    };

    write_rewrite_oracle_observation_fixture(temp.path(), &observation)
        .await
        .expect("fixture write should succeed");

    assert_eq!(
        fs::read_to_string(temp.path().join("stdout.txt")).expect("stdout"),
        "stdout\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("stderr.txt")).expect("stderr"),
        "stderr\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("exit_code.txt")).expect("exit"),
        "3\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("files/src/message.txt")).expect("artifact"),
        "hello\n"
    );
}

fn intentional(field: RewriteOracleField, reason: &str) -> RewriteOracleIntentionalIncompatibility {
    RewriteOracleIntentionalIncompatibility {
        field,
        reason: reason.to_string(),
    }
}
