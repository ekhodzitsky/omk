# Kimi Upstream Tracking

This page tracks the official Kimi docs OMK depends on before changing Kimi integration surfaces.

Last checked: 2026-05-19

## Tracked URLs

| Area | URL | Why it matters |
| --- | --- | --- |
| Docs root | <https://www.kimi.com/code/docs> | Starting point for upstream Kimi Code docs and release re-checks. |
| Wire Protocol | <https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html> | Source of truth for `kimi --wire`, JSON-RPC envelopes, handshake behavior, replay, events, and requests. |

## Observed Note

- Wire mode is `kimi --wire` and uses JSON-RPC 2.0 over stdin/stdout, with one JSON message per line.
- Local `kimi info` on 2026-05-09 reports Kimi CLI `1.41.0` and Wire protocol `1.9`.
- The official Wire Protocol page currently documents protocol version `1.9`; some request/response examples may still show older versions such as `1.7`.
- Kimi CLI 1.41.0 returns an object-shaped `initialize.result.hooks` payload with `supported_events` and `configured`, so OMK should treat handshake extension fields as structured JSON values rather than fixed arrays.
- Kimi CLI 1.41.0 streams event names such as `TurnBegin`, `ContentPart`, and `TurnEnd`; OMK normalizes event kinds before matching so PascalCase and snake_case forms remain compatible.
- Treat the current-version statement on the docs page as authoritative and re-check both sources before each release.

## Release Checklist

- [x] Re-open the official docs root and Wire Protocol page before release.
- [x] Re-check the local `kimi info` observation and record any protocol version change.
- [x] Update `README.md`, `SPEC.md`, `ROADMAP.md`, or `TODO.md` if upstream behavior changed.
- [x] Confirm `initialize`, `prompt`, `request`, `replay`, and fallback behavior still match the upstream docs.
