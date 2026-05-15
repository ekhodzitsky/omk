use std::path::{Component, Path};

pub(crate) fn is_safe_goal_agent_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "project files" {
        return true;
    }
    // Refuse any control character. Newlines or NULs inside a path are an
    // intent signal that the proposal is malformed or adversarial.
    if trimmed.chars().any(char::is_control) {
        return false;
    }
    // Refuse leading `~` so a downstream consumer cannot accidentally expand
    // a home reference on behalf of the agent.
    if trimmed.starts_with('~') {
        return false;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return false;
            }
            Component::Normal(part) => {
                let Some(part_str) = part.to_str() else {
                    return false;
                };
                // Reject any directory or file name that begins with `.git`
                // at any depth: `.git`, `.gitignore`, `.gitmodules`,
                // `.gitattributes`, `.github/`, `.gitlab-ci.yml`. Each of
                // these can pivot a benign "data write" proposal into code
                // execution through tooling that consumes those paths.
                if part_str.starts_with(".git") {
                    return false;
                }
            }
            Component::CurDir => {}
        }
    }
    true
}
