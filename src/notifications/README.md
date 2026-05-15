---
schema_version: 1
module: notifications
level: root
purpose: Send structured event notifications to external webhooks (Discord, Slack, Telegram).
status: stable
surface:
  - name: webhook
    kind: module
    visibility: pub
    contract: Webhook delivery subsystem containing config, dispatcher, and payload formatting.
    proof:
      kind: missing
      target: src/notifications/webhook/
      command: ""
  - name: send_notification
    kind: fn
    visibility: pub
    contract: Sends a NotificationEvent to all configured webhook destinations (Discord, Slack, Telegram). Failures are logged but not propagated.
    proof:
      kind: missing
      target: src/notifications/webhook/dispatcher.rs send_notification
      command: ""
  - name: NotificationEvent
    kind: enum
    visibility: pub
    contract: Tagged enum of all OMK runtime events that can trigger external notifications. Serialized with serde tag = "event".
    proof:
      kind: missing
      target: src/notifications/webhook/payload.rs NotificationEvent
      command: ""
  - name: WebhookConfig
    kind: struct
    visibility: pub
    contract: Optional per-destination webhook URLs. Used by runtime config to determine where to send notifications.
    proof:
      kind: missing
      target: src/notifications/webhook/config.rs WebhookConfig
      command: ""
dependencies:
  internal: []
  external:
    - name: serde
      scope: serialization of events and config
      reason: NotificationEvent and WebhookConfig derive Serialize/Deserialize.
    - name: reqwest
      scope: HTTP POST to webhook endpoints
      reason: Sends formatted payloads to Discord, Slack, and Telegram.
    - name: tracing
      scope: structured logging of delivery failures
      reason: Warnings emitted when a webhook request fails.
    - name: anyhow
      scope: error propagation in dispatcher
      reason: Context-rich errors for HTTP and serialization failures.
consumers:
  - path: src/cli/team/manage.rs
    uses: ["NotificationEvent::TeamShutdown"]
  - path: src/runtime/config.rs
    uses: ["WebhookConfig"]
  - path: src/runtime/session.rs
    uses: ["NotificationEvent", "send_notification"]
  - path: src/runtime/ralph/engine.rs
    uses: ["NotificationEvent::RalphComplete"]
  - path: src/runtime/autopilot/cli.rs
    uses: ["NotificationEvent::AutopilotComplete", "NotificationEvent::AutopilotFailed"]
  - path: src/runtime/ultrawork.rs
    uses: ["NotificationEvent::UltraworkComplete"]
invariants:
  - id: optional-destinations
    rule: Webhook URLs are optional; missing destinations are silently skipped without error.
    proof:
      kind: missing
      target: src/notifications/webhook/dispatcher.rs send_notification
      command: ""
  - id: http-timeout
    rule: All outbound webhook requests use a 30-second timeout to prevent hung connections.
    proof:
      kind: static-check
      target: src/notifications/webhook/dispatcher.rs
      command: "grep -n 'from_secs(30)' src/notifications/webhook/dispatcher.rs"
  - id: failure-non-fatal
    rule: Delivery failures are logged at warn level and do not propagate to the caller.
    proof:
      kind: missing
      target: src/notifications/webhook/dispatcher.rs send_notification
      command: ""
  - id: message-truncation
    rule: Platform-specific message fields are truncated to safe character limits (Discord 200/500, Slack 500, Telegram 500).
    proof:
      kind: static-check
      target: src/notifications/webhook/payload.rs
      command: "grep -n 'chars().take(' src/notifications/webhook/payload.rs"
verification:
  pre_change:
    - cargo test --lib notifications
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# notifications

## Architecture

The `notifications` module provides a thin, fire-and-forget bridge between OMK runtime events and external chat platforms. It is intentionally stateless: callers construct a `NotificationEvent`, pass it to `send_notification` along with a `WebhookConfig`, and the module handles formatting and delivery.

The module is organized as:

- **`webhook`** — A subsystem that owns destination configuration, payload formatting, and HTTP dispatch.
  - `config.rs` — `WebhookConfig` data shape.
  - `payload.rs` — `NotificationEvent` enum and per-platform formatters (`format_discord`, `format_slack`, `format_telegram`).
  - `dispatcher.rs` — `send_notification` entrypoint and async HTTP delivery.

The root `mod.rs` re-exports the three public items from `webhook` so callers do not need to know the internal file structure. The nested `webhook/mod.rs` currently acts as a proxy barrel; this is a known gap tracked in `TODO.md`.

## Files

| File | Responsibility |
|------|----------------|
| `mod.rs` | Module storefront; re-exports `send_notification`, `NotificationEvent`, `WebhookConfig`. |
| `webhook/mod.rs` | Proxy re-exports from config, dispatcher, and payload. |
| `webhook/config.rs` | `WebhookConfig` struct with optional per-destination URLs. |
| `webhook/dispatcher.rs` | Async delivery to Discord, Slack, and Telegram with timeouts and logging. |
| `webhook/payload.rs` | `NotificationEvent` enum and platform-specific message formatting. |
