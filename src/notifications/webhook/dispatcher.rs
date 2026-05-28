use tracing::{debug, warn};

use super::config::WebhookConfig;
use super::payload::{format_discord, format_slack, format_telegram, NotificationEvent};
use super::transport::{ReqwestWebhookTransport, WebhookTransport};
use crate::wire::protocol::scrub_secret_patterns;

/// Send a notification to all configured webhooks using the default
/// `ReqwestWebhookTransport`.
pub async fn send_notification(config: &WebhookConfig, event: &NotificationEvent) {
    send_notification_with_transport(config, event, &ReqwestWebhookTransport).await;
}

/// Send a notification to all configured webhooks through the given transport.
///
/// This is the trait-backed entrypoint. Production code uses
/// `send_notification`; tests inject `MockWebhookTransport` here.
pub async fn send_notification_with_transport(
    config: &WebhookConfig,
    event: &NotificationEvent,
    transport: &dyn WebhookTransport,
) {
    let payload = serde_json::to_string(event).unwrap_or_default();

    if let Some(url) = &config.discord {
        let (content, embeds) = format_discord(event);
        let body = serde_json::json!({
            "content": content,
            "embeds": embeds,
        });
        if let Err(e) = transport.post_json(url, body).await {
            warn!(url = %url, error = %e, "Failed to send Discord notification");
        }
    }

    if let Some(url) = &config.slack {
        let text = format_slack(event);
        let body = serde_json::json!({ "text": text });
        if let Err(e) = transport.post_json(url, body).await {
            warn!(url = %url, error = %e, "Failed to send Slack notification");
        }
    }

    if let Some(url) = &config.telegram {
        let text = format_telegram(event);
        let body = serde_json::json!({ "text": text, "parse_mode": "Markdown" });
        if let Err(e) = transport.post_json(url, body).await {
            warn!(url = %url, error = %e, "Failed to send Telegram notification");
        }
    }

    debug!(event = %scrub_secret_patterns(&payload), "Notification sent");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::webhook::transport::MockWebhookTransport;

    #[tokio::test]
    async fn send_notification_with_empty_config_does_nothing() {
        let config = WebhookConfig {
            discord: None,
            slack: None,
            telegram: None,
        };
        let event = NotificationEvent::Error {
            source: "test".to_string(),
            message: "hello".to_string(),
        };
        let transport = MockWebhookTransport::default();
        send_notification_with_transport(&config, &event, &transport).await;
        assert!(transport.calls.lock().await.is_empty());
    }

    #[tokio::test]
    async fn send_notification_with_transport_records_discord() {
        let event = NotificationEvent::Error {
            source: "test".to_string(),
            message: "hello".to_string(),
        };
        let transport = MockWebhookTransport::default();
        send_notification_with_transport(
            &WebhookConfig {
                discord: Some("https://discord.webhook/test".to_string()),
                slack: None,
                telegram: None,
            },
            &event,
            &transport,
        )
        .await;
        let calls = transport.calls.lock().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "https://discord.webhook/test");
    }

    #[tokio::test]
    async fn send_notification_serializes_event_to_debug_log() {
        let event = NotificationEvent::TeamSpawned {
            name: "test-team".to_string(),
            task: "test-task".to_string(),
            workers: 2,
            role: "qa".to_string(),
        };
        let payload = serde_json::to_string(&event).unwrap_or_default();
        assert!(!payload.is_empty());
        assert!(payload.contains("test-team"));
    }
}
