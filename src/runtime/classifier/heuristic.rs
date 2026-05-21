use crate::runtime::classifier::Intent;

const TRIVIAL_PREFIXES: &[&str] = &[
    "what is",
    "explain",
    "show",
    "how does",
    "where is",
    "does",
    "define",
    "summarise",
    "summary of",
];

#[derive(Debug)]
pub enum HeuristicOutcome {
    Empty,
    SlashCommand,
    Match(Intent, f32),
    Indeterminate,
}

pub fn heuristic_classify(prompt: &str) -> HeuristicOutcome {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return HeuristicOutcome::Empty;
    }
    if trimmed.starts_with('/') {
        return HeuristicOutcome::SlashCommand;
    }
    let normalized = trimmed.to_lowercase();
    let prefix_slice = if normalized.len() > 80 {
        &normalized[..80]
    } else {
        &normalized
    };
    for prefix in TRIVIAL_PREFIXES {
        if prefix_slice.starts_with(*prefix) && normalized.len() <= 80 {
            return HeuristicOutcome::Match(Intent::Trivial, 1.0);
        }
    }
    HeuristicOutcome::Indeterminate
}
