use std::path::PathBuf;

/// Sanitize a user-provided name for safe use in file paths.
///
/// Rejects names that could be used for path traversal or that are
/// otherwise invalid as directory/file names.
pub fn sanitize_name(name: &str) -> anyhow::Result<String> {
    if name.is_empty() {
        anyhow::bail!("name cannot be empty");
    }
    if name.len() > 64 {
        anyhow::bail!("name cannot be longer than 64 characters");
    }
    if name.starts_with('.') {
        anyhow::bail!("name cannot start with a dot");
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains(':') {
        anyhow::bail!("name contains invalid characters");
    }
    Ok(name.to_string())
}

/// Resolve a potentially user-provided path, ensuring it does not escape
/// the intended directory via path traversal.
pub fn resolve_safe_path(base: &std::path::Path, name: &str) -> anyhow::Result<PathBuf> {
    let sanitized = sanitize_name(name)?;
    let resolved = base.join(&sanitized);
    // Canonicalize the base to avoid symlink tricks, then verify the resolved
    // path is still under base.
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let canonical_resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    if !canonical_resolved.starts_with(&canonical_base) {
        anyhow::bail!("path escapes the intended directory");
    }
    Ok(resolved)
}
