use serde::{Deserialize, Serialize};

use super::estimator::CostEstimate;

/// Tracked cost for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCost {
    pub session_type: String,
    pub name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub estimate: CostEstimate,
    pub actual_usd: Option<f64>,
}
