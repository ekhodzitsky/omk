use std::path::{Component, Path};

use crate::runtime::goal::state::is_safe_goal_agent_path;
use super::GoalAgentTaskProposal;

const PROJECT_FILES_ALIAS: &str = "project files";

/// Check whether a single task proposal satisfies path and write-scope policy.
///
/// Returns `Some(reason)` if the proposal violates policy.
pub fn check_task_path_policy(proposal: &GoalAgentTaskProposal) -> Option<String> {
    if let Some(path) = proposal
        .read_set
        .iter()
        .chain(proposal.write_set.iter())
        .find(|path| !is_safe_goal_agent_path(path))
    {
        return Some(format!(
            "path is outside the allowed goal policy roots: {path}"
        ));
    }

    let policy_text = format!(
        "{} {} {}",
        proposal.id,
        proposal.description,
        proposal.write_set.join(" ")
    )
    .to_ascii_lowercase();
    if policy_text.contains("crates.io") || policy_text.contains("publish") {
        return Some("publishing is disabled for GitHub-only goal execution".to_string());
    }

    if proposal.budget_secs == 0 {
        return Some("task budget must be greater than zero".to_string());
    }

    None
}

pub(super) fn first_conflicting_path(candidate: &[String], accepted: &[String]) -> Option<String> {
    candidate.iter().find_map(|candidate_path| {
        accepted
            .iter()
            .find(|accepted_path| paths_conflict(candidate_path, accepted_path))
            .map(|_| display_goal_write_path(candidate_path))
    })
}

fn paths_conflict(candidate: &str, accepted: &str) -> bool {
    let Some(candidate) = normalize_goal_write_path(candidate) else {
        return false;
    };
    let Some(accepted) = normalize_goal_write_path(accepted) else {
        return false;
    };

    candidate == PROJECT_FILES_ALIAS
        || accepted == PROJECT_FILES_ALIAS
        || candidate == accepted
        || is_path_prefix(&candidate, &accepted)
        || is_path_prefix(&accepted, &candidate)
}

fn display_goal_write_path(path: &str) -> String {
    normalize_goal_write_path(path).unwrap_or_else(|| path.trim().to_string())
}

fn normalize_goal_write_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed == PROJECT_FILES_ALIAS {
        return Some(PROJECT_FILES_ALIAS.to_string());
    }

    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!parts.is_empty()).then(|| parts.join("/"))
}

fn is_path_prefix(parent: &str, child: &str) -> bool {
    child
        .strip_prefix(parent)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_curdir_segments() {
        assert_eq!(
            first_conflicting_path(&["./README.md".to_string()], &["README.md".to_string()]),
            Some("README.md".to_string())
        );
    }

    #[test]
    fn detects_parent_child_conflicts() {
        assert_eq!(
            first_conflicting_path(&["docs/guide.md".to_string()], &["docs".to_string()]),
            Some("docs/guide.md".to_string())
        );
    }

    #[test]
    fn does_not_treat_same_prefix_as_child_path() {
        assert_eq!(
            first_conflicting_path(&["docs2/guide.md".to_string()], &["docs".to_string()]),
            None
        );
    }
}
