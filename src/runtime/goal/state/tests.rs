use super::{
    format_goal_duration_secs, is_safe_goal_agent_path, normalize_goal, FileSystemGoalStateStore,
    GoalPhase, GoalStateStore, GoalStatus, GOAL_STATE_FILE,
};
use std::fs;

#[test]
fn goal_status_serializes_as_snake_case() {
    let value = serde_json::to_value(GoalStatus::NotReady).unwrap();
    assert_eq!(value, "not_ready");
}

#[test]
fn paused_goal_status_serializes_as_snake_case() {
    let value = serde_json::to_value(GoalStatus::Paused).unwrap();
    assert_eq!(value, "paused");
}

#[test]
fn goal_phase_serializes_as_snake_case() {
    let value = serde_json::to_value(GoalPhase::VerificationDesign).unwrap();
    assert_eq!(value, "verification_design");
}

#[test]
fn normalize_goal_collapses_whitespace() {
    assert_eq!(normalize_goal("  ship   it\nwell  "), "ship it well");
}

#[test]
fn goal_duration_formats_to_stable_compact_units() {
    assert_eq!(format_goal_duration_secs(0), "0s");
    assert_eq!(format_goal_duration_secs(59), "59s");
    assert_eq!(format_goal_duration_secs(60), "1m");
    assert_eq!(format_goal_duration_secs(3_600), "1h");
    assert_eq!(format_goal_duration_secs(86_400), "1d");
}

#[tokio::test]
async fn goal_state_loads_legacy_json_with_safe_defaults() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(GOAL_STATE_FILE),
        r#"{
              "goal_id": "goal-legacy",
              "original_goal": "Ship safely",
              "normalized_goal": "Ship safely",
              "status": "not_ready",
              "created_at": "2026-05-13T00:00:00Z",
              "updated_at": "2026-05-13T00:00:01Z"
            }"#,
    )
    .unwrap();

    let state = FileSystemGoalStateStore::new()
        .load(temp.path())
        .await
        .unwrap();

    assert_eq!(state.version, 1);
    assert_eq!(state.phase, GoalPhase::Intake);
    assert!(!state.until_ready);
    assert!(state.terminal_criteria.proof_required);
    assert!(state.terminal_criteria.gates_required);
    assert!(state.terminal_criteria.human_blockers_stop);
    assert!(state.artifacts.is_empty());
    assert_eq!(state.state_dir, temp.path());
}

#[tokio::test]
async fn goal_state_load_rehomes_stale_persisted_state_dir() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(GOAL_STATE_FILE),
        r#"{
              "version": 1,
              "goal_id": "goal-moved",
              "original_goal": "Resume after move",
              "normalized_goal": "Resume after move",
              "status": "paused",
              "phase": "proof",
              "created_at": "2026-05-13T00:00:00Z",
              "updated_at": "2026-05-13T00:00:01Z",
              "until_ready": true,
              "terminal_criteria": {
                "proof_required": true,
                "gates_required": true,
                "human_blockers_stop": true
              },
              "state_dir": "/old/machine/.local/state/omk/goals/goal-moved"
            }"#,
    )
    .unwrap();

    let state = FileSystemGoalStateStore::new()
        .load(temp.path())
        .await
        .unwrap();

    assert_eq!(state.goal_id, "goal-moved");
    assert_eq!(state.status, GoalStatus::Paused);
    assert_eq!(state.state_dir, temp.path());
}

#[test]
fn is_safe_goal_agent_path_accepts_typical_repo_paths() {
    for ok in [
        "project files",
        "README.md",
        "src/lib.rs",
        "tests/fixtures/data.json",
        "./Cargo.toml",
        "docs/architecture.md",
    ] {
        assert!(
            is_safe_goal_agent_path(ok),
            "expected '{ok}' to be accepted"
        );
    }
}

#[test]
fn is_safe_goal_agent_path_rejects_empty_and_absolute() {
    assert!(!is_safe_goal_agent_path(""));
    assert!(!is_safe_goal_agent_path("   "));
    assert!(!is_safe_goal_agent_path("/etc/passwd"));
    assert!(!is_safe_goal_agent_path("/Users/me/repo/src/lib.rs"));
}

#[test]
fn is_safe_goal_agent_path_rejects_traversal_variants() {
    assert!(!is_safe_goal_agent_path(".."));
    assert!(!is_safe_goal_agent_path("../etc"));
    assert!(!is_safe_goal_agent_path("../../escape"));
    assert!(!is_safe_goal_agent_path("foo/../bar"));
    assert!(!is_safe_goal_agent_path("src/../../escape"));
}

#[test]
fn is_safe_goal_agent_path_rejects_home_expansion_attempts() {
    // A literal tilde at the start is a strong intent signal that the
    // agent expects HOME expansion. Refuse it even though Rust file ops
    // would not expand it on their own.
    assert!(!is_safe_goal_agent_path("~/.bashrc"));
    assert!(!is_safe_goal_agent_path("~root/.ssh/authorized_keys"));
    assert!(!is_safe_goal_agent_path("~"));
}

#[test]
fn is_safe_goal_agent_path_rejects_control_characters() {
    assert!(!is_safe_goal_agent_path("README\n.md"));
    assert!(!is_safe_goal_agent_path("a\0b"));
    assert!(!is_safe_goal_agent_path("path\twith\ttabs"));
    assert!(!is_safe_goal_agent_path("foo\x07bar"));
}

#[test]
fn is_safe_goal_agent_path_rejects_dot_git_family_at_any_depth() {
    // First-component rejection: the historical .git/.git/ check is
    // preserved and extended to every dotfile a tool might consume.
    assert!(!is_safe_goal_agent_path(".git"));
    assert!(!is_safe_goal_agent_path(".git/config"));
    assert!(!is_safe_goal_agent_path(".gitignore"));
    assert!(!is_safe_goal_agent_path(".gitmodules"));
    assert!(!is_safe_goal_agent_path(".gitattributes"));
    // GitHub Actions metadata is a code-execution surface for CI.
    assert!(!is_safe_goal_agent_path(".github"));
    assert!(!is_safe_goal_agent_path(".github/workflows/ci.yml"));
    // GitLab CI metadata is the analogous surface for GitLab pipelines.
    assert!(!is_safe_goal_agent_path(".gitlab-ci.yml"));
    // Sub-component traversal: a path that smuggles `.git` deeper in the
    // tree (e.g., a submodule clone target) must still be rejected.
    assert!(!is_safe_goal_agent_path("vendor/.git/HEAD"));
    assert!(!is_safe_goal_agent_path("apps/web/.github/workflows/x.yml"));
}

#[test]
fn is_safe_goal_agent_path_preserves_special_alias() {
    // The `project files` alias is preserved across whitespace trimming
    // and stays exempt from per-path validation. Mixed-case variants are
    // not the alias and instead fall through to the regular path policy
    // (which accepts them as a literal relative file name).
    assert!(is_safe_goal_agent_path("project files"));
    assert!(is_safe_goal_agent_path("  project files  "));
}
