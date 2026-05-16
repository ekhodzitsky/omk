/// Lightweight goal-text heuristic that decomposes a goal into independent
/// feature names suitable for non-overlapping slice scopes.
///
/// Returns an empty vec when no meaningful decomposition is possible.
pub(crate) fn decompose_goal_for_slices(goal: &str, max_features: usize) -> Vec<String> {
    let normalized = goal.to_ascii_lowercase();
    // Try several split patterns, preferring the one that yields the most
    // balanced decomposition without exceeding max_features.
    let candidates = [
        split_on(&normalized, " and "),
        split_on(&normalized, ", and "),
        split_on(&normalized, ", "),
    ];

    let best = candidates
        .into_iter()
        .map(|mut c| {
            c.truncate(max_features);
            c
        })
        .filter(|c| c.len() > 1)
        .max_by_key(|c| c.iter().map(|s| s.len()).sum::<usize>());

    let mut features = best.unwrap_or_else(|| {
        // If no multi-part split worked, try extracting noun phrases after
        // action verbs like "with", "including", "plus".
        split_on_phrases(&normalized, max_features)
    });

    features.truncate(max_features);
    features.retain(|f| {
        let trimmed = f.trim();
        !trimmed.is_empty()
            && trimmed.len() > 2
            && !STOP_WORDS.contains(&trimmed)
            && !trimmed.starts_with("a ")
            && !trimmed.starts_with("an ")
            && !trimmed.starts_with("the ")
    });

    features
        .into_iter()
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect()
}

/// Sanitize a feature name into a filesystem-safe directory slug.
pub(crate) fn sanitize_feature_slug(feature: &str) -> String {
    feature
        .to_ascii_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

fn split_on(text: &str, delimiter: &str) -> Vec<String> {
    text.split(delimiter)
        .map(|s| s.trim().to_string())
        .collect()
}

fn split_on_phrases(text: &str, max_features: usize) -> Vec<String> {
    let delimiters = [" with ", " including ", " plus ", " featuring "];
    for delim in &delimiters {
        if let Some(pos) = text.find(delim) {
            let prefix = text[..pos].trim().to_string();
            let suffix = text[pos + delim.len()..].trim().to_string();
            let sub = split_on(&suffix, " and ")
                .into_iter()
                .chain(split_on(&suffix, ", "))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if !sub.is_empty() && sub.len() < max_features {
                let mut result = vec![prefix];
                result.extend(sub.into_iter().take(max_features - 1));
                return result;
            }
        }
    }
    Vec::new()
}

static STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "with", "for", "to", "of", "in", "on", "at", "by",
    "from", "as", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do",
    "does", "did", "will", "would", "could", "should", "may", "might", "must", "can", "this",
    "that", "these", "those", "it", "its", "i", "you", "he", "she", "we", "they",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompose_and() {
        let features = decompose_goal_for_slices("Build a CLI with config parsing and logging", 4);
        assert_eq!(features, vec!["build a cli with config parsing", "logging"]);
    }

    #[test]
    fn test_decompose_comma_and() {
        let features = decompose_goal_for_slices("Add OAuth, rate limiting, and audit logging", 4);
        assert_eq!(
            features,
            vec!["add oauth", "rate limiting", "and audit logging"]
        );
    }

    #[test]
    fn test_decompose_respects_max_features() {
        let features =
            decompose_goal_for_slices("Alpha and Beta and Gamma and Delta and Epsilon", 3);
        assert_eq!(features.len(), 3);
        assert_eq!(features, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_decompose_single_feature_returns_empty() {
        let features = decompose_goal_for_slices("Build a simple CLI", 4);
        assert!(features.is_empty());
    }

    #[test]
    fn test_sanitize_feature_slug() {
        assert_eq!(sanitize_feature_slug("config parsing"), "config_parsing");
        assert_eq!(sanitize_feature_slug("OAuth 2.0"), "oauth_20");
        assert_eq!(sanitize_feature_slug("Rate-Limiting!"), "ratelimiting");
    }
}
