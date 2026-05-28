use anyhow::Result;

use crate::cost::estimator::CostEstimate;
use crate::notifications::{send_notification, NotificationEvent};
use crate::runtime::session::SessionSummary;

/// Record session cost and send a notification webhook.
pub async fn record_session_end(
    summary: &SessionSummary,
    estimate: CostEstimate,
    notification: NotificationEvent,
) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir();
    let sink = crate::cost::file_sink::JsonFileCostSink::new(state_dir.join("costs.json"));
    let tracker = crate::cost::tracker::CostTracker::new(sink);
    let _ = tracker
        .record(crate::cost::types::SessionCost {
            session_type: summary.session_type.clone(),
            name: summary.name.clone(),
            started_at: summary.started_at,
            ended_at: Some(summary.ended_at),
            estimate,
            actual_usd: None,
        })
        .await;

    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();
    if let Some(webhooks) = config.webhooks {
        send_notification(&webhooks, &notification).await;
    }

    Ok(())
}
