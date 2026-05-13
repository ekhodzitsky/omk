use omk::runtime::goal::{plan_goal_worktree, plan_goal_worktrees};
use std::path::Path;

#[test]
fn test_goal_worktree_plan_is_deterministic_and_bead_scoped() {
    let root = Path::new("/repo/.omk/worktrees");

    let first = plan_goal_worktree(root, "goal-20260513-155000-deadbeef", "omk-io2.2")
        .expect("worktree plan should be valid");
    let second = plan_goal_worktree(root, "goal-20260513-155000-deadbeef", "omk-io2.2")
        .expect("worktree plan should be repeatable");

    assert_eq!(first, second);
    assert_eq!(first.goal_id, "goal-20260513-155000-deadbeef");
    assert_eq!(first.task_id, "omk-io2.2");
    assert_eq!(
        first.branch_name,
        "omk/goal/goal-20260513-155000-deadbeef/omk-io2-2-7ec701dc2d4c52a1"
    );
    assert_eq!(
        first.worktree_name,
        "goal-goal-20260513-155000-deadbeef-omk-io2-2-7ec701dc2d4c52a1"
    );
    assert_eq!(first.worktree_path, root.join(&first.worktree_name));
}

#[test]
fn test_goal_worktree_plan_normalizes_unsafe_components() {
    let root = Path::new("/repo/.omk/worktrees");

    let plan = plan_goal_worktree(root, " ../Goal MVP ", "omk/io2:2")
        .expect("unsafe separators should normalize into safe names");

    assert_eq!(plan.goal_component, "goal-mvp");
    assert_eq!(plan.task_component, "omk-io2-2");
    assert!(!plan.branch_name.contains(".."));
    assert!(!plan.branch_name.contains(':'));
    assert!(!plan.branch_name.contains('\\'));
    assert!(!plan.worktree_name.contains('/'));
    assert!(!plan.worktree_name.contains('\\'));
}

#[test]
fn test_goal_worktree_plan_rejects_components_without_safe_text() {
    let root = Path::new("/repo/.omk/worktrees");

    let err = plan_goal_worktree(root, "../..", "omk-io2.2")
        .expect_err("path traversal only should not become a component");

    assert!(err.to_string().contains("goal id"));
}

#[test]
fn test_goal_worktree_plan_rejects_control_characters() {
    let root = Path::new("/repo/.omk/worktrees");

    let err = plan_goal_worktree(root, "goal-1", "task\n1")
        .expect_err("control characters should not normalize into identifiers");

    assert!(err.to_string().contains("task id"));
}

#[test]
fn test_goal_worktree_plan_avoids_normalized_identifier_collisions() {
    let root = Path::new("/repo/.omk/worktrees");

    let slash = plan_goal_worktree(root, "Goal MVP", "agent/implement")
        .expect("slash task should normalize");
    let colon = plan_goal_worktree(root, "Goal MVP", "agent:implement")
        .expect("colon task should normalize");

    assert_eq!(slash.goal_component, colon.goal_component);
    assert_eq!(slash.task_component, colon.task_component);
    assert_ne!(slash.branch_name, colon.branch_name);
    assert_ne!(slash.worktree_name, colon.worktree_name);
    assert_ne!(slash.worktree_path, colon.worktree_path);
}

#[test]
fn test_goal_worktree_batch_planner_rejects_duplicate_collisions() {
    let root = Path::new("/repo/.omk/worktrees");

    let err = plan_goal_worktrees(root, "Goal MVP", ["omk-io2.2", "omk-io2.2"])
        .expect_err("duplicate task plans should collide");

    assert!(err.to_string().contains("worktree plan collision"));
}
