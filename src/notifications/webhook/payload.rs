use serde::Serialize;

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

pub(super) fn format_discord(event: &NotificationEvent) -> (String, Vec<serde_json::Value>) {
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

pub(super) fn format_slack(event: &NotificationEvent) -> String {
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

pub(super) fn format_telegram(event: &NotificationEvent) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_discord_team_spawned() {
        let event = NotificationEvent::TeamSpawned {
            name: "alpha".to_string(),
            task: "fix bugs".to_string(),
            workers: 3,
            role: "coder".to_string(),
        };
        let (content, embeds) = format_discord(&event);
        assert!(content.contains("Team spawned"));
        assert_eq!(embeds.len(), 1);
        let embed = &embeds[0];
        assert_eq!(embed["title"], "Team Spawned");
        assert_eq!(embed["color"], 0x22c55e);
    }

    #[test]
    fn format_discord_team_shutdown_success() {
        let event = NotificationEvent::TeamShutdown {
            name: "alpha".to_string(),
            duration_secs: 42,
            status: "success".to_string(),
        };
        let (content, embeds) = format_discord(&event);
        assert!(content.contains("Team shutdown"));
        assert_eq!(embeds[0]["color"], 0x22c55e);
    }

    #[test]
    fn format_discord_team_shutdown_failure() {
        let event = NotificationEvent::TeamShutdown {
            name: "alpha".to_string(),
            duration_secs: 42,
            status: "failed".to_string(),
        };
        let (_content, embeds) = format_discord(&event);
        assert_eq!(embeds[0]["color"], 0xef4444);
    }

    #[test]
    fn format_slack_team_spawned() {
        let event = NotificationEvent::TeamSpawned {
            name: "beta".to_string(),
            task: "refactor".to_string(),
            workers: 2,
            role: "architect".to_string(),
        };
        let text = format_slack(&event);
        assert!(text.contains("Team Spawned"));
        assert!(text.contains("beta"));
        assert!(text.contains("architect"));
        assert!(text.contains("2"));
    }

    #[test]
    fn format_slack_autopilot_failed_truncates_error() {
        let long_error = "x".repeat(600);
        let event = NotificationEvent::AutopilotFailed {
            name: "gamma".to_string(),
            phase: "test".to_string(),
            error: long_error.clone(),
        };
        let text = format_slack(&event);
        assert!(text.contains("Autopilot Failed"));
        assert!(!text.contains(&"x".repeat(600))); // should be truncated
    }

    #[test]
    fn format_telegram_contains_markdown() {
        let event = NotificationEvent::UltraworkComplete {
            jobs_total: 10,
            jobs_success: 8,
            duration_secs: 120,
        };
        let text = format_telegram(&event);
        assert!(text.contains("Ultrawork Complete"));
        assert!(text.contains("8/10"));
    }

    #[test]
    fn format_discord_fallback_for_unmatched_variant() {
        let event = NotificationEvent::AutopilotStarted {
            name: "delta".to_string(),
            task: "ship".to_string(),
        };
        let (content, embeds) = format_discord(&event);
        assert!(content.contains("AutopilotStarted"));
        assert!(embeds.is_empty());
    }

    #[test]
    fn notification_event_serializes_with_tag() {
        let event = NotificationEvent::Error {
            source: "test".to_string(),
            message: "boom".to_string(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "Error");
        assert_eq!(json["source"], "test");
        assert_eq!(json["message"], "boom");
    }
}
