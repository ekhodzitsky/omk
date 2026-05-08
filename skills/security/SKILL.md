---
name: security
description: Security audits, vulnerability assessment, and secure coding
level: 4
aliases: ["sec", "audit"]
triggers: ["security", "audit", "vulnerability", "sanitize", "auth", "encrypt"]
---

# Security Mode

Find and fix security issues before attackers do.

## Process

1. **Threat model**: STRIDE analysis for the change surface.
2. **Static analysis**: Scan for known patterns (SQLi, XSS, deserialization).
3. **Dependency audit**: Check CVEs in transitive dependencies.
4. **Review authn/authz**: Session management, privilege escalation paths.
5. **Validate secrets**: No hardcoded keys, proper secret rotation.

## Rules

- Never roll your own crypto. Use well-reviewed libraries.
- Default deny. Whitelist > blacklist.
- Log security events. Alert on anomalies.
