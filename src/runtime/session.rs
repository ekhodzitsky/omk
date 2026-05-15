use anyhow::Result;
use chrono::{DateTime, Utc};

/// Record session cost and send a notification webhook.
pub async fn record_session_end(
    session_type: &str,
    name: &str,
    started_at: DateTime<Utc>,
    estimate: crate::cost::estimator::CostEstimate,
    notification: crate::notifications::NotificationEvent,
) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir();
    let sink = crate::cost::file_sink::JsonFileCostSink::new(state_dir.join("costs.json"));
    let tracker = crate::cost::tracker::CostTracker::new(sink);
    let _ = tracker
        .record(crate::cost::types::SessionCost {
            session_type: session_type.to_string(),
            name: name.to_string(),
            started_at,
            ended_at: Some(Utc::now()),
            estimate,
            actual_usd: None,
        })
        .await;

    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();
    if let Some(webhooks) = config.webhooks {
        crate::notifications::send_notification(&webhooks, &notification).await;
    }

    Ok(())
}
