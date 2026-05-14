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
    // Canonicalize the base to neutralize symlink tricks. When the resolved
    // child does not exist yet (the common create-a-new-file case), fall back
    // to canonical_base.join(sanitized) so the containment check uses the
    // same canonical prefix on both sides — otherwise a symlinked base such
    // as `/var` vs `/private/var` would cause a false reject.
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let canonical_resolved = resolved
        .canonicalize()
        .unwrap_or_else(|_| canonical_base.join(&sanitized));
    if !canonical_resolved.starts_with(&canonical_base) {
        anyhow::bail!("path escapes the intended directory");
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn sanitize_name_rejects_empty() {
        assert!(sanitize_name("").is_err());
    }

    #[test]
    fn sanitize_name_rejects_overlong() {
        let too_long = "a".repeat(65);
        assert!(sanitize_name(&too_long).is_err());
    }

    #[test]
    fn sanitize_name_accepts_max_length() {
        // The cutoff is `> 64`, so exactly 64 bytes must pass.
        let at_limit = "a".repeat(64);
        assert_eq!(sanitize_name(&at_limit).unwrap(), at_limit);
    }

    #[test]
    fn sanitize_name_rejects_dot_leading() {
        // Includes the bare `.` and `..` traversal tokens.
        assert!(sanitize_name(".").is_err());
        assert!(sanitize_name("..").is_err());
        assert!(sanitize_name(".hidden").is_err());
        assert!(sanitize_name(".env").is_err());
    }

    #[test]
    fn sanitize_name_rejects_embedded_traversal() {
        // `..` anywhere in the name is a traversal hint we refuse to host.
        assert!(sanitize_name("foo..bar").is_err());
        assert!(sanitize_name("a..").is_err());
        assert!(sanitize_name("safe..name").is_err());
    }

    #[test]
    fn sanitize_name_rejects_path_separators() {
        assert!(sanitize_name("foo/bar").is_err());
        assert!(sanitize_name("foo\\bar").is_err());
        assert!(sanitize_name("/abs").is_err());
        assert!(sanitize_name("trailing/").is_err());
    }

    #[test]
    fn sanitize_name_rejects_colon() {
        // Colon is a drive separator on Windows and an alternate data stream
        // marker on NTFS; rejecting it keeps the helper cross-platform.
        assert!(sanitize_name("foo:bar").is_err());
        assert!(sanitize_name("C:name").is_err());
    }

    #[test]
    fn sanitize_name_accepts_typical_identifiers() {
        for ok in ["hello", "hello-world_123", "file.txt", "a"] {
            assert_eq!(sanitize_name(ok).unwrap(), ok);
        }
    }

    #[test]
    fn resolve_safe_path_joins_with_base() {
        // Even when the base does not exist on disk, the resolved path must
        // be the literal join — canonicalize is best-effort and falls back.
        let base = Path::new("/tmp/omk-sanitize-test-nonexistent");
        let path = resolve_safe_path(base, "child").unwrap();
        assert_eq!(path, base.join("child"));
    }

    #[test]
    fn resolve_safe_path_rejects_invalid_names() {
        // The first gate is `sanitize_name`, so all of its rejections must
        // surface as `resolve_safe_path` errors too.
        let base = Path::new("/tmp");
        assert!(resolve_safe_path(base, "..").is_err());
        assert!(resolve_safe_path(base, "../etc").is_err());
        assert!(resolve_safe_path(base, ".hidden").is_err());
        assert!(resolve_safe_path(base, "a/b").is_err());
        assert!(resolve_safe_path(base, "").is_err());
        assert!(resolve_safe_path(base, &"x".repeat(65)).is_err());
    }

    #[test]
    fn resolve_safe_path_stays_under_existing_base() {
        // When the base exists on disk, the canonicalize guard must agree
        // that a sanitized child remains under it.
        let base = std::env::temp_dir();
        let path = resolve_safe_path(&base, "child").unwrap();
        assert!(path.starts_with(&base));
    }
}
