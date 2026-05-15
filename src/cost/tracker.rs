use anyhow::Result;

use super::sink::CostSink;
use super::types::SessionCost;

/// Persistent cost tracker backed by a [`CostSink`].
///
/// `CostTracker` knows nothing about files or I/O. All storage operations
/// are delegated to the generic `S: CostSink` implementation, making the
/// tracker fully testable with an in-memory backend.
pub struct CostTracker<S: CostSink> {
    sink: S,
}

impl<S: CostSink> CostTracker<S> {
    pub fn new(sink: S) -> Self {
        Self { sink }
    }

    pub async fn record(&self, cost: SessionCost) -> Result<()> {
        let mut costs = self.sink.load().await?;
        costs.push(cost);
        self.sink.save(&costs).await?;
        Ok(())
    }

    pub async fn total_estimated(&self) -> Result<f64> {
        let costs = self.sink.load().await?;
        Ok(costs.iter().map(|c| c.estimate.estimated_usd).sum())
    }

    pub async fn report(&self) -> Result<String> {
        let costs = self.sink.load().await?;
        if costs.is_empty() {
            return Ok("No cost data recorded yet.".to_string());
        }

        let total: f64 = costs.iter().map(|c| c.estimate.estimated_usd).sum();
        let by_type = costs.iter().fold(
            std::collections::HashMap::new(),
            |mut acc, c| {
                *acc.entry(c.session_type.clone()).or_insert(0.0) += c.estimate.estimated_usd;
                acc
            },
        );

        let mut report = "💰 OMK Cost Report\n".to_string();
        report.push_str(&format!("Total estimated: ~${:.4}\n\n", total));
        report.push_str("By session type:\n");
        for (t, amount) in by_type {
            report.push_str(&format!("  {:20} ${:.4}\n", t, amount));
        }
        report.push_str(&format!("\nSessions: {}\n", costs.len()));

        Ok(report)
    }

    /// Clear all recorded costs.
    pub async fn clear(&self) -> Result<()> {
        self.sink.save(&[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::estimator::{CostEstimate, PricingTier};
    use crate::cost::sink::InMemoryCostSink;
    use crate::cost::types::SessionCost;
    use chrono::Utc;

    fn sample_estimate(usd: f64) -> CostEstimate {
        CostEstimate {
            input_tokens: 1000,
            output_tokens: 500,
            duration_secs: 60,
            worker_count: 1,
            estimated_usd: usd,
            tier: PricingTier::Standard,
        }
    }

    fn sample_cost(session_type: &str, usd: f64) -> SessionCost {
        SessionCost {
            session_type: session_type.to_string(),
            name: "test-session".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            estimate: sample_estimate(usd),
            actual_usd: None,
        }
    }

    #[tokio::test]
    async fn test_record_and_total() {
        let sink = InMemoryCostSink::new();
        let tracker = CostTracker::new(sink);

        tracker.record(sample_cost("team", 1.23)).await.unwrap();
        tracker.record(sample_cost("team", 2.77)).await.unwrap();

        let total = tracker.total_estimated().await.unwrap();
        assert!((total - 4.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_report_empty() {
        let sink = InMemoryCostSink::new();
        let tracker = CostTracker::new(sink);

        let report = tracker.report().await.unwrap();
        assert!(report.contains("No cost data recorded yet"));
    }

    #[tokio::test]
    async fn test_report_grouping() {
        let sink = InMemoryCostSink::new();
        let tracker = CostTracker::new(sink);

        tracker.record(sample_cost("team", 1.0)).await.unwrap();
        tracker.record(sample_cost("autopilot", 2.0)).await.unwrap();
        tracker.record(sample_cost("team", 3.0)).await.unwrap();

        let report = tracker.report().await.unwrap();
        assert!(report.contains("Total estimated: ~$6.0000"));
        assert!(report.contains("team"));
        assert!(report.contains("autopilot"));
        assert!(report.contains("Sessions: 3"));
    }

    #[tokio::test]
    async fn test_clear() {
        let sink = InMemoryCostSink::new();
        let tracker = CostTracker::new(sink);

        tracker.record(sample_cost("team", 5.0)).await.unwrap();
        tracker.clear().await.unwrap();

        let total = tracker.total_estimated().await.unwrap();
        assert!((total).abs() < f64::EPSILON);
    }
}
