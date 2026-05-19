# notifications — Agent Guide

## Editing Rules

1. **Webhooks are fire-and-forget.** `send_notification` logs failures but never
   propagates them to the caller. A broken webhook must not block a goal or
   gate from completing.
2. **Transport is a trait boundary.** `WebhookTransport` (or `reqwest::Client`
   wrapped in a local trait) is the only place HTTP calls happen. Pure payload
   formatting lives in `webhook/payload.rs` and must not import `reqwest`.
3. **No secrets in logs.** Webhook URLs contain tokens. Scrub them with
   `scrub_secret_patterns` before any `tracing` event or debug output.
4. **Idempotent payloads.** A `NotificationEvent` may be retried. Payload
   builders must produce the same JSON body for the same event.
5. **Test through mocks.** Unit tests exercise `send_notification` with an
   in-memory `WebhookTransport` that records dispatched payloads. No real HTTP.
