pub mod estimator;
pub mod file_sink;
pub mod sink;
pub mod tracker;
pub mod types;

pub use estimator::CostEstimate;
pub use file_sink::JsonFileCostSink;
pub use sink::CostSink;
pub use tracker::CostTracker;
pub use types::SessionCost;

/// Heuristic USD estimate from token count.
///
/// Approximates $2 / 1M tokens.
pub fn estimated_usd_from_tokens(tokens: u64) -> f64 {
    tokens as f64 * 0.000002
}
