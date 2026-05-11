# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :white_check_mark: |
| 0.1.x   | :white_check_mark: |
| < 0.1.0 | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in oh-my-kimi, please report it responsibly:

1. **Do not open a public issue.**
2. Email **ekhodzitsky@gmail.com** with:
   - A description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

We will acknowledge receipt within 48 hours and provide a timeline for a fix.

## Security Considerations

- `omk` runs local commands and Kimi processes. Always validate input with `validate_safe()` before passing data to shell-command helpers.
- `omk` uses `shlex::try_quote` for shell escaping. Do not bypass this.
- State files may contain sensitive task descriptions. Ensure `~/.local/state/omk/` has appropriate permissions (`0700`).
