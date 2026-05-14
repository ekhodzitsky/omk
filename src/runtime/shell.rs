use anyhow::Result;

/// Escape a string for safe inclusion in a POSIX shell command.
///
/// This uses the `shlex` crate to properly handle:
/// - Dollar signs -> no expansion
/// - Backslashes -> preserved
/// - Newlines -> preserved within quotes
/// - Empty strings -> "''"
/// - Strings containing single quotes -> wrapped in double quotes
///   (or the `'\''` idiom when shell metacharacters force it)
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

    #[test]
    fn shell_escape_roundtrip_extra_dangerous_inputs() {
        // Additional edge cases beyond the basic roundtrip set: characters
        // that would change meaning if unquoted (redirects, globs, mixed
        // quoting, escapes, embedded tabs, non-ASCII), and a payload that
        // mixes several at once.
        let cases = [
            "> /etc/passwd",
            "< /dev/null",
            "* glob ?",
            "&& malicious",
            "|| fallback",
            r"path\with\backslashes",
            "it's \"double\" too",
            "a\tb",
            "café — мир 🌍",
            "mix: $X `cmd` 'q' \"q\" \\ \t end",
        ];

        for s in cases {
            let escaped = shell_escape(s).expect("shell_escape must succeed for safe text");
            let parsed = shlex::split(&format!("cmd {escaped}"));
            assert_eq!(
                parsed,
                Some(vec!["cmd".to_string(), s.to_string()]),
                "roundtrip failed for input: {s:?}",
            );
        }
    }

    #[test]
    fn shell_escape_rejects_null_byte() {
        // shlex::try_quote refuses NUL bytes; this is the documented escape
        // hatch surfaced as an error rather than a silently broken command.
        assert!(shell_escape("foo\0bar").is_err());
    }

    #[test]
    fn validate_safe_allows_intended_whitespace() {
        // \n and \t are the two intentionally allow-listed control bytes;
        // regular spaces and printable punctuation must pass too.
        assert!(validate_safe("").is_ok());
        assert!(validate_safe("hello world").is_ok());
        assert!(validate_safe("line1\nline2").is_ok());
        assert!(validate_safe("col1\tcol2").is_ok());
        assert!(validate_safe("mix\nof\tboth").is_ok());
    }

    #[test]
    fn validate_safe_rejects_other_control_chars() {
        // Every non-allow-listed ASCII control byte must be rejected,
        // including \r, vertical tab, form feed, BEL, ESC, and DEL.
        for byte in 0u8..=31 {
            if byte == b'\n' || byte == b'\t' {
                continue;
            }
            let s = format!("a{}b", byte as char);
            assert!(
                validate_safe(&s).is_err(),
                "byte 0x{byte:02x} must be rejected",
            );
        }
        // DEL (0x7F) is also classified as ASCII control.
        assert!(validate_safe("a\x7fb").is_err());
    }

    #[test]
    fn validate_safe_allows_non_ascii_unicode() {
        // Multi-byte UTF-8 sequences must not be misread as control bytes;
        // every continuation byte has the top bit set and is >= 0x80.
        assert!(validate_safe("héllo мир 🌍").is_ok());
    }
}
