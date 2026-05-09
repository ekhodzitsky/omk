use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Supported webhook destinations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub discord: Option<String>,
    pub slack: Option<String>,
    pub telegram: Option<String>,
}

/// Events that trigger notifications.
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
#[serde(tag = "event")]
pub enum NotificationEvent {
    TeamSpawned {
        name: String,
        task: String,
        workers: usize,
        role: String,
    },
    TeamShutdown {
        name: String,
        duration_secs: u64,
        status: String,
    },
    AutopilotStarted {
        name: String,
        task: String,
    },
    AutopilotComplete {
        name: String,
        duration_secs: u64,
        phases_completed: usize,
    },
    AutopilotFailed {
        name: String,
        phase: String,
        error: String,
    },
    RalphIteration {
        name: String,
        iteration: usize,
        max_iterations: usize,
        verified: usize,
        total: usize,
    },
    RalphComplete {
        name: String,
        duration_secs: u64,
        iterations: usize,
        verified: usize,
        total: usize,
    },
    UltraworkComplete {
        jobs_total: usize,
        jobs_success: usize,
        duration_secs: u64,
    },
    Error {
        source: String,
        message: String,
    },
}

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
        .context("Discord webhook request failed")?;

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
        .context("Slack webhook request failed")?;

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
        .context("Telegram webhook request failed")?;

    Ok(())
}

fn format_discord(event: &NotificationEvent) -> (String, Vec<serde_json::Value>) {
    match event {
        NotificationEvent::TeamSpawned {
            name,
            task,
            workers,
            role,
        } => {
            let embed = serde_json::json!({
                "title": "Team Spawned",
                "color": 0x22c55e,
                "fields": [
                    { "name": "Name", "value": name, "inline": true },
                    { "name": "Role", "value": role, "inline": true },
                    { "name": "Workers", "value": workers.to_string(), "inline": true },
                    { "name": "Task", "value": task.chars().take(200).collect::<String>() },
                ]
            });
            ("🚀 Team spawned".to_string(), vec![embed])
        }
        NotificationEvent::TeamShutdown {
            name,
            duration_secs,
            status,
        } => {
            let embed = serde_json::json!({
                "title": "Team Shutdown",
                "color": if status == "success" { 0x22c55e } else { 0xef4444 },
                "fields": [
                    { "name": "Name", "value": name, "inline": true },
                    { "name": "Status", "value": status, "inline": true },
                    { "name": "Duration", "value": format!("{}s", duration_secs), "inline": true },
                ]
            });
            ("🛑 Team shutdown".to_string(), vec![embed])
        }
        NotificationEvent::AutopilotComplete {
            name,
            duration_secs,
            phases_completed,
        } => {
            let embed = serde_json::json!({
                "title": "Autopilot Complete",
                "color": 0x22c55e,
                "fields": [
                    { "name": "Name", "value": name, "inline": true },
                    { "name": "Duration", "value": format!("{}s", duration_secs), "inline": true },
                    { "name": "Phases", "value": phases_completed.to_string(), "inline": true },
                ]
            });
            ("🤖 Autopilot complete".to_string(), vec![embed])
        }
        NotificationEvent::RalphComplete {
            name,
            duration_secs,
            iterations,
            verified,
            total,
        } => {
            let embed = serde_json::json!({
                "title": "Ralph Complete",
                "color": 0x22c55e,
                "fields": [
                    { "name": "Name", "value": name, "inline": true },
                    { "name": "Duration", "value": format!("{}s", duration_secs), "inline": true },
                    { "name": "Iterations", "value": iterations.to_string(), "inline": true },
                    { "name": "Stories", "value": format!("{}/{}", verified, total), "inline": true },
                ]
            });
            ("🔄 Ralph complete".to_string(), vec![embed])
        }
        NotificationEvent::UltraworkComplete {
            jobs_total,
            jobs_success,
            duration_secs,
        } => {
            let embed = serde_json::json!({
                "title": "Ultrawork Complete",
                "color": 0x3b82f6,
                "fields": [
                    { "name": "Jobs", "value": format!("{}/{}", jobs_success, jobs_total), "inline": true },
                    { "name": "Duration", "value": format!("{}s", duration_secs), "inline": true },
                ]
            });
            ("⚡ Ultrawork complete".to_string(), vec![embed])
        }
        NotificationEvent::AutopilotFailed { name, phase, error } => {
            let embed = serde_json::json!({
                "title": "Autopilot Failed",
                "color": 0xef4444,
                "fields": [
                    { "name": "Name", "value": name, "inline": true },
                    { "name": "Phase", "value": phase, "inline": true },
                    { "name": "Error", "value": error.chars().take(500).collect::<String>() },
                ]
            });
            ("❌ Autopilot failed".to_string(), vec![embed])
        }
        NotificationEvent::Error { source, message } => {
            let embed = serde_json::json!({
                "title": "OMK Error",
                "color": 0xef4444,
                "fields": [
                    { "name": "Source", "value": source, "inline": true },
                    { "name": "Message", "value": message.chars().take(500).collect::<String>() },
                ]
            });
            ("❌ Error".to_string(), vec![embed])
        }
        _ => (format!("{:?}", event), vec![]),
    }
}

fn format_slack(event: &NotificationEvent) -> String {
    match event {
        NotificationEvent::TeamSpawned {
            name,
            task,
            workers,
            role,
        } => {
            format!(
                "🚀 *Team Spawned*\n• Name: {}\n• Role: {}\n• Workers: {}\n• Task: {}",
                name,
                role,
                workers,
                task.chars().take(200).collect::<String>()
            )
        }
        NotificationEvent::TeamShutdown {
            name,
            duration_secs,
            status,
        } => {
            format!(
                "🛑 *Team Shutdown*\n• Name: {}\n• Status: {}\n• Duration: {}s",
                name, status, duration_secs
            )
        }
        NotificationEvent::AutopilotComplete {
            name,
            duration_secs,
            phases_completed,
        } => {
            format!(
                "🤖 *Autopilot Complete*\n• Name: {}\n• Duration: {}s\n• Phases: {}",
                name, duration_secs, phases_completed
            )
        }
        NotificationEvent::RalphComplete {
            name,
            duration_secs,
            iterations,
            verified,
            total,
        } => {
            format!("🔄 *Ralph Complete*\n• Name: {}\n• Duration: {}s\n• Iterations: {}\n• Stories: {}/{}",
                name, duration_secs, iterations, verified, total)
        }
        NotificationEvent::UltraworkComplete {
            jobs_total,
            jobs_success,
            duration_secs,
        } => {
            format!(
                "⚡ *Ultrawork Complete*\n• Jobs: {}/{}\n• Duration: {}s",
                jobs_success, jobs_total, duration_secs
            )
        }
        NotificationEvent::AutopilotFailed { name, phase, error } => {
            format!(
                "❌ *Autopilot Failed*\n• Name: {}\n• Phase: {}\n• Error: {}",
                name,
                phase,
                error.chars().take(500).collect::<String>()
            )
        }
        NotificationEvent::Error { source, message } => {
            format!(
                "❌ *Error*\n• Source: {}\n• Message: {}",
                source,
                message.chars().take(500).collect::<String>()
            )
        }
        _ => format!("{:?}", event),
    }
}

fn format_telegram(event: &NotificationEvent) -> String {
    match event {
        NotificationEvent::TeamSpawned {
            name,
            task,
            workers,
            role,
        } => {
            format!(
                "🚀 *Team Spawned*\n*Name:* `{}`\n*Role:* {}\n*Workers:* {}\n*Task:* {}",
                name,
                role,
                workers,
                task.chars().take(200).collect::<String>()
            )
        }
        NotificationEvent::TeamShutdown {
            name,
            duration_secs,
            status,
        } => {
            format!(
                "🛑 *Team Shutdown*\n*Name:* `{}`\n*Status:* {}\n*Duration:* {}s",
                name, status, duration_secs
            )
        }
        NotificationEvent::AutopilotComplete {
            name,
            duration_secs,
            phases_completed,
        } => {
            format!(
                "🤖 *Autopilot Complete*\n*Name:* `{}`\n*Duration:* {}s\n*Phases:* {}",
                name, duration_secs, phases_completed
            )
        }
        NotificationEvent::RalphComplete {
            name,
            duration_secs,
            iterations,
            verified,
            total,
        } => {
            format!("🔄 *Ralph Complete*\n*Name:* `{}`\n*Duration:* {}s\n*Iterations:* {}\n*Stories:* {}/{}",
                name, duration_secs, iterations, verified, total)
        }
        NotificationEvent::UltraworkComplete {
            jobs_total,
            jobs_success,
            duration_secs,
        } => {
            format!(
                "⚡ *Ultrawork Complete*\n*Jobs:* {}/{}\n*Duration:* {}s",
                jobs_success, jobs_total, duration_secs
            )
        }
        NotificationEvent::AutopilotFailed { name, phase, error } => {
            format!(
                "❌ *Autopilot Failed*\n*Name:* `{}`\n*Phase:* {}\n*Error:* {}",
                name,
                phase,
                error.chars().take(500).collect::<String>()
            )
        }
        NotificationEvent::Error { source, message } => {
            format!(
                "❌ *Error*\n*Source:* `{}`\n*Message:* {}",
                source,
                message.chars().take(500).collect::<String>()
            )
        }
        _ => format!("{:?}", event),
    }
}
