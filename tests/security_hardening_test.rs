//! Integration coverage for the security-hardening pass.
//!
//! These tests pin the public-facing safety contracts that the unit
//! tests inside each module already exercise. Keeping a separate
//! integration suite makes it easy for a future reviewer to audit
//! "is this still hardened?" without grepping every module — and it
//! drives the helpers through their published API surface, the way a
//! downstream library consumer would.

use omk::runtime::goal::{plan_goal_worktree, plan_goal_worktrees};
use omk::runtime::sanitize::{resolve_safe_path, sanitize_name};
use omk::runtime::shell::{shell_escape, validate_safe};
use omk::wire::redact_wire_secrets;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn sanitize_name_blocks_traversal_and_metacharacters() {
    // Every rejection here corresponds to a previously-observed agent
    // proposal shape we never want to materialise on disk. They run
    // through the public helper so the integration boundary stays honest
    // even if the inner module is refactored.
    let blocked = [
        "..",
        "../etc",
        ".hidden",
        "/abs",
        "foo/bar",
        "foo\\bar",
        "foo:bar",
        "",
        &"x".repeat(65),
    ];
    for name in blocked {
        assert!(
            sanitize_name(name).is_err(),
            "sanitize_name must reject {name:?}",
        );
    }
}

#[test]
fn resolve_safe_path_keeps_files_inside_an_existing_base() {
    let base = tempdir().unwrap();
    let resolved = resolve_safe_path(base.path(), "child.txt").unwrap();
    assert!(resolved.starts_with(base.path()));
    assert_eq!(
        resolved.file_name().and_then(|s| s.to_str()),
        Some("child.txt")
    );
}

#[test]
fn resolve_safe_path_rejects_attempted_traversal_even_with_existing_base() {
    let base = tempdir().unwrap();
    for attempted in ["../escape.txt", "/abs/escape", ".hidden"] {
        assert!(
            resolve_safe_path(base.path(), attempted).is_err(),
            "resolve_safe_path must reject {attempted:?}",
        );
    }
}

#[test]
fn shell_escape_round_trips_through_shlex_split() {
    // The escape contract is "anything you pass in comes out as one
    // shell token, byte-for-byte". This is the property advisor / gate
    // rendering relies on for the human-display path.
    let cases = [
        "",
        "plain",
        "with spaces",
        "$HOME",
        "`rm -rf /`",
        "foo; rm -rf /",
        "foo | cat /etc/passwd",
        "line1\nline2",
        r"path\with\backslashes",
        "café — мир 🌍",
    ];
    for s in cases {
        let escaped = shell_escape(s).expect("shell_escape must succeed for safe text");
        let parsed = shlex::split(&format!("cmd {escaped}"));
        assert_eq!(
            parsed,
            Some(vec!["cmd".to_string(), s.to_string()]),
            "shell_escape round-trip lost data for {s:?}",
        );
    }
}

#[test]
fn validate_safe_rejects_unprintable_control_bytes() {
    assert!(validate_safe("hello\0world").is_err());
    assert!(validate_safe("ring-the-bell\x07").is_err());
    assert!(validate_safe("escape\x1bme").is_err());
    // Tabs and newlines are explicitly allowed for multi-line evidence.
    assert!(validate_safe("col1\tcol2").is_ok());
    assert!(validate_safe("line1\nline2").is_ok());
}

#[test]
fn goal_worktree_plan_is_deterministic_and_safe() {
    let worktrees_root = std::path::Path::new("/tmp/omk-security-hardening-worktrees");
    let plan_a = plan_goal_worktree(worktrees_root, "goal-abc", "task-1").unwrap();
    let plan_b = plan_goal_worktree(worktrees_root, "goal-abc", "task-1").unwrap();
    assert_eq!(
        plan_a, plan_b,
        "worktree plans must be stable for the same inputs"
    );
    assert!(
        plan_a.branch_name.starts_with("omk/goal/")
            && !plan_a.branch_name.contains("..")
            && !plan_a.branch_name.contains(' ')
            && !plan_a.branch_name.contains('\t')
            && !plan_a.branch_name.contains('\0'),
        "branch name must stay inside the sanctioned namespace: {}",
        plan_a.branch_name,
    );
    assert!(
        !plan_a.worktree_name.contains('/'),
        "worktree directory name must remain a single path component: {}",
        plan_a.worktree_name,
    );
}

#[test]
fn goal_worktree_plan_rejects_adversarial_identifiers() {
    let worktrees_root = std::path::Path::new("/tmp/omk-security-hardening-worktrees");
    // Empty / control / collision inputs all converge to either an
    // explicit error or a sanitized component that stays a single path
    // segment. We refuse the empty case outright.
    assert!(plan_goal_worktree(worktrees_root, "", "task").is_err());
    assert!(plan_goal_worktree(worktrees_root, "goal", "").is_err());
    // Slash and traversal characters get normalised into dashes; the
    // sanctioned namespace prefix must still hold.
    let plan =
        plan_goal_worktree(worktrees_root, "goal/../escape", "task::weird").expect("normalised");
    assert!(
        plan.branch_name.starts_with("omk/goal/"),
        "branch name must stay inside the sanctioned namespace: {}",
        plan.branch_name,
    );
    assert!(!plan.worktree_name.contains('/'));
    assert!(!plan.worktree_name.contains('\\'));
    // Plans across multiple tasks must remain unique (collision detection).
    let plans =
        plan_goal_worktrees(worktrees_root, "goal-x", ["task-1", "task-2", "task-3"]).unwrap();
    let mut branches: Vec<_> = plans.iter().map(|p| p.branch_name.clone()).collect();
    branches.sort();
    branches.dedup();
    assert_eq!(branches.len(), 3, "task plans must produce unique branches");
}

#[test]
fn redact_wire_secrets_scrubs_token_shapes_inside_strings() {
    // End-to-end: an agent transcript that quotes a leaked GitHub PAT
    // and AWS access key inside a plain message field. The redaction
    // pipeline must replace both fragments without consuming the
    // surrounding narrative so the durable log remains readable.
    //
    // Each fixture is assembled at runtime from harmless fragments so the
    // source file never contains a contiguous string that matches a real
    // GitHub / AWS token signature. GitHub push-protection would otherwise
    // reject this test file as a leaked credential.
    let github_pat = ["ghp", "_", "abcdefghijklmnop1234567890abcdef0011"].concat();
    let aws_key = ["AKIA", "ABCDEFGHIJKLMNOP"].concat();
    let raw = json!({
        "transcript": [
            format!("secret leak: {github_pat} was committed"),
            format!("deploy used key {aws_key} for staging"),
            "no secrets here — just discussing api key rotation policy".to_string(),
        ]
    });
    let redacted = redact_wire_secrets(&raw);
    let transcript = redacted["transcript"].as_array().unwrap();
    assert_eq!(transcript[0], "secret leak: [REDACTED] was committed");
    assert_eq!(transcript[1], "deploy used key [REDACTED] for staging");
    assert_eq!(
        transcript[2],
        "no secrets here — just discussing api key rotation policy",
    );
}
