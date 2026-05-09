# Kimi Upstream Tracking

This page tracks the official Kimi docs OMK depends on before changing Kimi integration surfaces.

Last checked: 2026-05-09

## Tracked URLs

| Area | URL | Why it matters |
| --- | --- | --- |
| Docs root | <https://www.kimi.com/code/docs> | Starting point for upstream Kimi Code docs and release re-checks. |
| Wire Protocol | <https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html> | Source of truth for `kimi --wire`, JSON-RPC envelopes, handshake behavior, replay, events, and requests. |

## Observed Note

- Wire mode is `kimi --wire` and uses JSON-RPC 2.0 over stdin/stdout, with one JSON message per line.
- A repository note from 2026-05-08 recorded `kimi info` on Kimi CLI 1.41.0 as reporting Wire protocol `1.9`.
- The official Wire Protocol page currently documents protocol version `1.9`; some request/response examples may still show older versions such as `1.7`.
- Treat the current-version statement on the docs page as authoritative and re-check both sources before each release.

## Release Checklist

- [ ] Re-open the official docs root and Wire Protocol page before release.
- [ ] Re-check the local `kimi info` observation and record any protocol version change.
- [ ] Update `README.md`, `SPEC.md`, `ROADMAP.md`, or `TODO.md` if upstream behavior changed.
- [ ] Confirm `initialize`, `prompt`, `request`, `replay`, and fallback behavior still match the upstream docs.
