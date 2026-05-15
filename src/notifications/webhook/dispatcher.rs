use anyhow::{Context, Result};
use tracing::{debug, warn};

use super::config::WebhookConfig;
use super::payload::{format_discord, format_slack, format_telegram, NotificationEvent};

/// Send a notification to all configured webhooks.
pub async fn send_notification(config: &WebhookConfig, event: &NotificationEvent) {
    let payload = serde_json::to_string(event).unwrap_or_default();

    if let Some(url) = &config.discord {
        if let Err(e) = send_discord(url, event).await {
            warn!(url = %url, error = %e, "Failed to send Discord notification");
        }
    }

    if let Some(url) = &config.slack {
        if let Err(e) = send_slack(url, event).await {
            warn!(url = %url, error = %e, "Failed to send Slack notification");
        }
    }

    if let Some(url) = &config.telegram {
        if let Err(e) = send_telegram(url, event).await {
            warn!(url = %url, error = %e, "Failed to send Telegram notification");
        }
    }

    debug!(event = %payload, "Notification sent");
}

async fn send_discord(url: &str, event: &NotificationEvent) -> Result<()> {
    let (content, embeds) = format_discord(event);
    let body = serde_json::json!({
        "content": content,
        "embeds": embeds,
    });

    reqwest::Client::new()
        .post(url)
        .timeout(std::time::Duration::from_secs(30))
        .json(&body)
        .send()
        .await
        .context("Discord webhook request failed")?
        .error_for_status()
        .context("Discord webhook returned an error status")?;

    Ok(())
}

async fn send_slack(url: &str, event: &NotificationEvent) -> Result<()> {
    let text = format_slack(event);
    let body = serde_json::json!({ "text": text });

    reqwest::Client::new()
        .post(url)
        .timeout(std::time::Duration::from_secs(30))
        .json(&body)
        .send()
        .await
        .context("Slack webhook request failed")?
        .error_for_status()
        .context("Slack webhook returned an error status")?;

    Ok(())
}

async fn send_telegram(url: &str, event: &NotificationEvent) -> Result<()> {
    // Telegram bot API: url is like https://api.telegram.org/bot<token>/sendMessage
    // with chat_id in the URL or we require the full URL with chat_id query param
    let text = format_telegram(event);
    let body = serde_json::json!({ "text": text, "parse_mode": "Markdown" });

    reqwest::Client::new()
        .post(url)
        .timeout(std::time::Duration::from_secs(30))
        .json(&body)
        .send()
        .await
        .context("Telegram webhook request failed")?
        .error_for_status()
        .context("Telegram webhook returned an error status")?;

    Ok(())
}
