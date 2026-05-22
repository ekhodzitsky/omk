use super::registry::COMMAND_REGISTRY;

/// Return command names (and aliases) that match the given prefix.
///
/// * If `prefix` starts with `'/'` the slash is stripped before matching.
/// * Matching is case-insensitive.
/// * Results are sorted alphabetically and deduplicated.
pub fn complete(prefix: &str) -> Vec<&'static str> {
    let needle = prefix.strip_prefix('/').unwrap_or(prefix).to_lowercase();

    let mut out = Vec::new();
    for spec in COMMAND_REGISTRY {
        if spec.name.to_lowercase().starts_with(&needle) {
            out.push(spec.name);
        }
        for alias in spec.aliases {
            if alias.to_lowercase().starts_with(&needle) {
                out.push(alias);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Return subcommand completions for `/show`.
pub fn completions_for_show(prefix: &str) -> Vec<&'static str> {
    let candidates = ["plan", "proof", "goals"];
    let needle = prefix.to_lowercase();
    candidates
        .iter()
        .filter(|c| c.to_lowercase().starts_with(&needle))
        .copied()
        .collect()
}
