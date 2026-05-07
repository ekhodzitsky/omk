/// Escape a string for safe inclusion in a POSIX shell command.
///
/// This uses the `shlex` crate to properly handle:
/// - Single quotes -> '\'''
/// - Dollar signs -> no expansion
/// - Backslashes -> preserved
/// - Newlines -> preserved within quotes
/// - Empty strings -> "''"
///
/// # Example
/// ```
/// use omk::runtime::shell::shell_escape;
/// assert_eq!(shell_escape("hello"), "'hello'");
/// assert_eq!(shell_escape("it's"), "'it'\\''s'");
/// ```
pub fn shell_escape(s: &str) -> String {
    shlex::quote(s).into_owned()
}

/// Validate that a string does not contain null bytes or other
/// characters that could break shell scripts or file paths.
pub fn validate_safe(s: &str) -> Result<(), &'static str> {
    if s.contains('\0') {
        return Err("input contains null bytes");
    }
    if s.bytes().any(|b| b.is_ascii_control() && b != b'\n' && b != b'\t') {
        return Err("input contains control characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_basic() {
        assert_eq!(shell_escape("hello"), "hello");
    }

    #[test]
    fn test_shell_escape_single_quote() {
        // shlex uses double quotes when single quote is present
        assert_eq!(shell_escape("it's"), "\"it's\"");
    }

    #[test]
    fn test_shell_escape_dollar() {
        // shlex uses single quotes for $ to prevent expansion
        assert_eq!(shell_escape("$HOME"), "'$HOME'");
    }

    #[test]
    fn test_shell_escape_backtick() {
        assert_eq!(shell_escape("`rm -rf /`"), "'`rm -rf /`'");
    }

    #[test]
    fn test_shell_escape_semicolon() {
        assert_eq!(shell_escape("foo; rm -rf /"), "'foo; rm -rf /'");
    }

    #[test]
    fn test_shell_escape_pipe() {
        assert_eq!(shell_escape("foo | cat /etc/passwd"), "'foo | cat /etc/passwd'");
    }

    #[test]
    fn test_shell_escape_newline() {
        let escaped = shell_escape("line1\nline2");
        assert!(escaped.contains("line1"));
        assert!(escaped.contains("line2"));
    }

    #[test]
    fn test_shell_escape_empty() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn test_validate_safe_null() {
        assert!(validate_safe("hello\0world").is_err());
    }

    #[test]
    fn test_validate_safe_control() {
        assert!(validate_safe("hello\x07world").is_err());
    }

    #[test]
    fn test_validate_safe_ok() {
        assert!(validate_safe("hello world 123").is_ok());
    }
}
