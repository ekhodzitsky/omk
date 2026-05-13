use std::fs;
use std::path::{Path, PathBuf};

mod runtime_goal {
    pub(crate) mod state {
        pub(crate) fn normalize_goal(goal: &str) -> String {
            goal.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }

    pub(crate) mod oracle {
        include!("../src/runtime/goal/oracle.rs");
    }
}

use runtime_goal::oracle::rewrite::{
    compare_rewrite_oracle, RewriteOracleField, RewriteOracleObservation,
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

#[test]
fn rewrite_oracle_accepts_matching_command_and_file_artifacts() {
    let source_fixture = fixture_path("rewrite_tiny_source_project");
    assert!(source_fixture.join("src/message.txt").exists());

    let expected = rewrite_fixture("rewrite_expected_observation");
    let actual = rewrite_fixture("rewrite_actual_matching_observation");

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
    assert!(comparison
        .mismatches
        .iter()
        .any(|mismatch| mismatch.field == RewriteOracleField::Stdout));
    assert!(comparison
        .mismatches
        .iter()
        .any(|mismatch| mismatch.field == RewriteOracleField::Stderr));
    assert!(comparison
        .mismatches
        .iter()
        .any(|mismatch| mismatch.field == RewriteOracleField::ExitCode));
    assert!(comparison.mismatches.iter().any(|mismatch| {
        mismatch.field
            == RewriteOracleField::FileArtifact {
                path: "src/message.txt".to_string(),
            }
    }));
}
