use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewriteOracleObservation {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: i32,
    pub(crate) file_artifacts: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewriteOracleComparison {
    pub(crate) compatible: bool,
    pub(crate) mismatches: Vec<RewriteOracleMismatch>,
    pub(crate) intentional_incompatibilities: Vec<RewriteOracleIntentionalIncompatibility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewriteOracleMismatch {
    pub(crate) field: RewriteOracleField,
    pub(crate) expected: String,
    pub(crate) actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewriteOracleIntentionalIncompatibility {
    pub(crate) field: RewriteOracleField,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RewriteOracleField {
    Stdout,
    Stderr,
    ExitCode,
    FileArtifact { path: String },
}

pub(crate) async fn run_rewrite_oracle_command(
    project_dir: &Path,
    command: &str,
    args: &[&str],
    artifact_paths: &[&str],
    timeout_duration: Duration,
) -> Result<RewriteOracleObservation> {
    let mut child = Command::new(command);
    child.current_dir(project_dir).args(args);
    crate::runtime::shell::configure_command(&mut child);
    let output = timeout(timeout_duration, child.output())
        .await
        .with_context(|| format!("Timed out while running rewrite oracle command: {command}"))?
        .with_context(|| format!("Failed to run rewrite oracle command: {command}"))?;
    let file_artifacts = read_file_artifacts(project_dir, artifact_paths).await?;
    Ok(observation_from_output(output, file_artifacts))
}

pub(crate) fn compare_rewrite_oracle(
    expected: &RewriteOracleObservation,
    actual: &RewriteOracleObservation,
) -> RewriteOracleComparison {
    let mut mismatches = Vec::new();

    compare_text_field(
        RewriteOracleField::Stdout,
        &expected.stdout,
        &actual.stdout,
        &mut mismatches,
    );
    compare_text_field(
        RewriteOracleField::Stderr,
        &expected.stderr,
        &actual.stderr,
        &mut mismatches,
    );

    if expected.exit_code != actual.exit_code {
        mismatches.push(RewriteOracleMismatch {
            field: RewriteOracleField::ExitCode,
            expected: expected.exit_code.to_string(),
            actual: actual.exit_code.to_string(),
        });
    }

    compare_file_artifacts(
        &expected.file_artifacts,
        &actual.file_artifacts,
        &mut mismatches,
    );

    RewriteOracleComparison {
        compatible: mismatches.is_empty(),
        mismatches,
        intentional_incompatibilities: Vec::new(),
    }
}

pub(crate) fn compare_rewrite_oracle_with_intentional_incompatibilities(
    expected: &RewriteOracleObservation,
    actual: &RewriteOracleObservation,
    intentional: &[RewriteOracleIntentionalIncompatibility],
) -> RewriteOracleComparison {
    let comparison = compare_rewrite_oracle(expected, actual);
    let mut mismatches = Vec::new();
    let mut intentional_incompatibilities = Vec::new();

    for mismatch in comparison.mismatches {
        if let Some(accepted) = intentional
            .iter()
            .find(|accepted| accepted.field == mismatch.field)
        {
            intentional_incompatibilities.push(accepted.clone());
        } else {
            mismatches.push(mismatch);
        }
    }

    RewriteOracleComparison {
        compatible: mismatches.is_empty(),
        mismatches,
        intentional_incompatibilities,
    }
}

pub(crate) async fn write_rewrite_oracle_observation_fixture(
    fixture_dir: &Path,
    observation: &RewriteOracleObservation,
) -> Result<()> {
    tokio::fs::create_dir_all(fixture_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to create rewrite fixture: {}",
                fixture_dir.display()
            )
        })?;
    write_fixture_file(fixture_dir.join("stdout.txt"), &observation.stdout).await?;
    write_fixture_file(fixture_dir.join("stderr.txt"), &observation.stderr).await?;
    write_fixture_file(
        fixture_dir.join("exit_code.txt"),
        &format!("{}\n", observation.exit_code),
    )
    .await?;
    for (relative, contents) in &observation.file_artifacts {
        write_fixture_file(fixture_dir.join("files").join(relative), contents).await?;
    }
    Ok(())
}

fn observation_from_output(
    output: Output,
    file_artifacts: Vec<(String, String)>,
) -> RewriteOracleObservation {
    RewriteOracleObservation {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        file_artifacts,
    }
}

async fn read_file_artifacts(
    project_dir: &Path,
    artifact_paths: &[&str],
) -> Result<Vec<(String, String)>> {
    let mut artifacts = Vec::new();
    for relative in artifact_paths {
        let path = project_dir.join(relative);
        if !path.starts_with(project_dir) {
            anyhow::bail!(
                "artifact path escapes project directory: {}",
                path.display()
            );
        }
        let contents = tokio::fs::read_to_string(&path).await.with_context(|| {
            format!("Failed to read rewrite oracle artifact: {}", path.display())
        })?;
        artifacts.push((relative.to_string(), contents));
    }
    Ok(artifacts)
}

fn compare_text_field(
    field: RewriteOracleField,
    expected: &str,
    actual: &str,
    mismatches: &mut Vec<RewriteOracleMismatch>,
) {
    if expected != actual {
        mismatches.push(RewriteOracleMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
}

fn compare_file_artifacts(
    expected: &[(String, String)],
    actual: &[(String, String)],
    mismatches: &mut Vec<RewriteOracleMismatch>,
) {
    let expected = artifact_map(expected);
    let actual = artifact_map(actual);
    let paths: BTreeSet<_> = expected.keys().chain(actual.keys()).copied().collect();

    for path in paths {
        match (expected.get(path), actual.get(path)) {
            (Some(expected), Some(actual)) if expected != actual => {
                mismatches.push(artifact_mismatch(path, expected, actual));
            }
            (Some(expected), None) => {
                mismatches.push(artifact_mismatch(path, expected, "<missing>"));
            }
            (None, Some(actual)) => {
                mismatches.push(artifact_mismatch(path, "<missing>", actual));
            }
            _ => {}
        }
    }
}

fn artifact_map(artifacts: &[(String, String)]) -> BTreeMap<&str, &str> {
    artifacts
        .iter()
        .map(|(path, contents)| (path.as_str(), contents.as_str()))
        .collect()
}

fn artifact_mismatch(path: &str, expected: &str, actual: &str) -> RewriteOracleMismatch {
    RewriteOracleMismatch {
        field: RewriteOracleField::FileArtifact {
            path: path.to_string(),
        },
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

async fn write_fixture_file(path: PathBuf, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .context("rewrite oracle fixture file must have a parent")?;
    tokio::fs::create_dir_all(parent).await.with_context(|| {
        format!(
            "Failed to create rewrite fixture directory: {}",
            parent.display()
        )
    })?;
    tokio::fs::write(&path, contents.as_bytes())
        .await
        .with_context(|| format!("Failed to write rewrite fixture file: {}", path.display()))
}
