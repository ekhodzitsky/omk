use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::analysis::{find_function_definitions, parse_file};

/// Extract lowercase keywords from a goal description, filtering common stop words.
fn goal_keywords(goal: &str) -> HashSet<String> {
    goal.to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty() && s.len() > 2 && !STOP_WORDS.contains(&s.as_str()))
        .collect()
}

static STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "can", "had", "her", "was", "one",
    "our", "out", "day", "get", "has", "him", "his", "how", "man", "new", "now", "old", "see",
    "two", "way", "who", "boy", "did", "its", "let", "put", "say", "she", "too", "use", "with",
    "have", "this", "will", "your", "from", "they", "know", "want", "been", "good", "much", "some",
    "time", "very", "when", "come", "here", "just", "like", "long", "make", "many", "over", "such",
    "take", "than", "them", "well", "were", "into", "only", "also", "each", "does", "done", "down",
    "first", "after", "back", "other", "many", "then", "these", "think", "where", "being", "every",
    "great", "might", "shall", "still", "those", "would", "should", "could", "about", "before",
    "right", "through", "while", "place", "made", "live", "where", "found", "work", "call", "give",
    "most", "must", "need", "same", "seem", "turn", "hand", "high", "sure", "upon", "head", "help",
    "home", "side", "move", "both", "five", "once", "same", "part", "keep", "last", "find", "more",
    "next", "open", "play", "read", "show", "stop", "tell", "very", "want", "what",
];

/// Walk the project directory, parse source files with tree-sitter, and return
/// paths whose function names match keywords extracted from the goal description.
///
/// Results are ordered by relevance (number of matching functions), most relevant first.
pub fn discover_relevant_files(goal: &str, project_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let keywords = goal_keywords(goal);
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    let mut scores: HashMap<PathBuf, usize> = HashMap::new();

    for entry in walkdir::WalkDir::new(project_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if should_skip(path, project_dir) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(tree) = parse_file(path, &source) else {
            continue;
        };
        let funcs = find_function_definitions(&tree);
        let score = funcs
            .iter()
            .filter(|f| {
                let name_lower = f.name.to_ascii_lowercase();
                keywords.iter().any(|kw| name_lower.contains(kw))
            })
            .count();
        if score > 0 {
            *scores.entry(path.to_path_buf()).or_insert(0) += score;
        }
    }

    let mut scored: Vec<(PathBuf, usize)> = scores.into_iter().collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.1));
    Ok(scored.into_iter().map(|(path, _)| path).collect())
}

fn should_skip(path: &Path, project_dir: &Path) -> bool {
    let relative = path.strip_prefix(project_dir).unwrap_or(path);
    let components: Vec<std::borrow::Cow<'_, str>> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();
    if components.iter().any(|c| c.starts_with('.')) {
        return true;
    }
    if components.iter().any(|c| {
        *c == "target" || *c == "node_modules" || *c == "vendor" || *c == "dist" || *c == "build"
    }) {
        return true;
    }
    !matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs")
            | Some("js")
            | Some("jsx")
            | Some("mjs")
            | Some("ts")
            | Some("tsx")
            | Some("py")
            | Some("pyi")
            | Some("go")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_discover_relevant_files_finds_matching_function() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let src_dir = temp.path().join("src");
        std::fs::create_dir(&src_dir)?;

        let mut file = std::fs::File::create(src_dir.join("parser.rs"))?;
        writeln!(file, "fn parse_input() {{}}")?;
        drop(file);

        let mut file2 = std::fs::File::create(src_dir.join("main.rs"))?;
        writeln!(file2, "fn main() {{}}")?;
        drop(file2);

        let results = discover_relevant_files("Implement parse_input logic", temp.path())?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], src_dir.join("parser.rs"));
        Ok(())
    }

    #[test]
    fn test_discover_relevant_files_skips_hidden_and_target() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let target_dir = temp.path().join("target").join("debug");
        std::fs::create_dir_all(&target_dir)?;
        let mut file = std::fs::File::create(target_dir.join("dump.rs"))?;
        writeln!(file, "fn parse_input() {{}}")?;
        drop(file);

        let results = discover_relevant_files("Implement parse_input logic", temp.path())?;
        assert!(results.is_empty());
        Ok(())
    }

    #[test]
    fn test_discover_relevant_files_empty_for_no_match() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let src_dir = temp.path().join("src");
        std::fs::create_dir(&src_dir)?;
        let mut file = std::fs::File::create(src_dir.join("lib.rs"))?;
        writeln!(file, "fn helper() {{}}")?;
        drop(file);

        let results = discover_relevant_files("xyz nonexistent", temp.path())?;
        assert!(results.is_empty());
        Ok(())
    }
}
