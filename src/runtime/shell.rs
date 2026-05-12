use anyhow::Result;

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
/// assert_eq!(shell_escape("hello").unwrap(), "hello");
/// assert_eq!(shell_escape("it's").unwrap(), "\"it's\"");
/// assert_eq!(shell_escape("$HOME").unwrap(), "'$HOME'");
/// ```
pub fn shell_escape(s: &str) -> Result<String> {
    Ok(shlex::try_quote(s)
        .map_err(|e| anyhow::anyhow!("failed to quote shell string: {e}"))?
        .into_owned())
}

/// Validate that a string does not contain null bytes or other
/// characters that could break shell scripts or file paths.
pub fn validate_safe(s: &str) -> Result<(), &'static str> {
    if s.contains('\0') {
        return Err("input contains null bytes");
    }
    if s.bytes()
        .any(|b| b.is_ascii_control() && b != b'\n' && b != b'\t')
    {
        return Err("input contains control characters");
    }
    Ok(())
}

/// Run an external command with retry and rate-limit backoff.
///
/// Uses [`crate::runtime::retry::RetryConfig::default`] and forwards
/// stderr to the retry logic so that HTTP 429 / rate-limit responses
/// trigger a longer fixed delay instead of exponential backoff.
#[allow(dead_code)]
pub async fn run_command_with_retry(cmd: &mut tokio::process::Command) -> anyhow::Result<String> {
    crate::runtime::retry::retry_command(crate::runtime::retry::RetryConfig::default(), cmd).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_basic() {
        assert_eq!(shell_escape("hello").unwrap(), "hello");
    }

    #[test]
    fn test_shell_escape_single_quote() {
        // shlex uses double quotes when single quote is present
        assert_eq!(shell_escape("it's").unwrap(), "\"it's\"");
    }

    #[test]
    fn test_shell_escape_dollar() {
        // shlex uses single quotes for $ to prevent expansion
        assert_eq!(shell_escape("$HOME").unwrap(), "'$HOME'");
    }

    #[test]
    fn test_shell_escape_backtick() {
        assert_eq!(shell_escape("`rm -rf /`").unwrap(), "'`rm -rf /`'");
    }

    #[test]
    fn test_shell_escape_semicolon() {
        assert_eq!(shell_escape("foo; rm -rf /").unwrap(), "'foo; rm -rf /'");
    }

    #[test]
    fn test_shell_escape_pipe() {
        assert_eq!(
            shell_escape("foo | cat /etc/passwd").unwrap(),
            "'foo | cat /etc/passwd'"
        );
    }

    #[test]
    fn test_shell_escape_newline() {
        let escaped = shell_escape("line1\nline2").unwrap();
        assert!(escaped.contains("line1"));
        assert!(escaped.contains("line2"));
    }

    #[test]
    fn test_shell_escape_empty() {
        assert_eq!(shell_escape("").unwrap(), "''");
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

    #[tokio::test]
    async fn test_run_command_with_retry_success() {
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("hello");
        let result = run_command_with_retry(&mut cmd).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[test]
    fn shell_escape_roundtrip() {
        let cases = [
            "",
            "hello",
            "with spaces",
            "it's quoted",
            "$HOME",
            "`rm -rf /`",
            "foo | cat /etc/passwd",
            "semi;colon",
            "line1\nline2",
            r#"json {"key":"value"}"#,
        ];

        for s in cases {
            let escaped = shell_escape(s).unwrap();
            let parsed = shlex::split(&format!("cmd {escaped}"));
            assert_eq!(parsed, Some(vec!["cmd".to_string(), s.to_string()]));
        }
    }
}
