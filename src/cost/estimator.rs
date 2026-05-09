#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Pricing tiers for heuristic cost estimation.
/// These are approximate rates based on typical API pricing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum PricingTier {
    /// Budget tier (~$2 / 1M tokens output)
    Budget,
    /// Standard tier (~$8 / 1M tokens output)
    #[default]
    Standard,
    /// Premium tier (~$24 / 1M tokens output)
    Premium,
}

impl PricingTier {
    pub fn dollars_per_1m_tokens(&self) -> f64 {
        match self {
            PricingTier::Budget => 2.0,
            PricingTier::Standard => 8.0,
            PricingTier::Premium => 24.0,
        }
    }

    pub fn from_model_hint(hint: &str) -> Self {
        match hint.to_lowercase().as_str() {
            "kimi" | "claude" | "gpt-4" => PricingTier::Standard,
            "codex" | "gpt-4o" => PricingTier::Premium,
            "gemini" | "haiku" => PricingTier::Budget,
            _ => PricingTier::Standard,
        }
    }
}

/// Estimated cost breakdown for a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Estimated input tokens
    pub input_tokens: u64,
    /// Estimated output tokens
    pub output_tokens: u64,
    /// Estimated duration in seconds
    pub duration_secs: u64,
    /// Number of workers/agents
    pub worker_count: usize,
    /// Estimated cost in USD
    pub estimated_usd: f64,
    /// Pricing tier used
    pub tier: PricingTier,
}

impl CostEstimate {
    pub fn formatted(&self) -> String {
        format!(
            "~${:.4} ({} workers, ~{}s, ~{}K tokens)",
            self.estimated_usd,
            self.worker_count,
            self.duration_secs,
            (self.input_tokens + self.output_tokens) / 1000
        )
    }
}

/// Estimate cost based on heuristics.
///
/// Heuristics:
/// - ~1000 tokens/minute per worker for active generation
/// - Input:output ratio ~ 3:1
/// - Minimum 500 tokens per session
pub fn estimate_cost(
    duration_secs: u64,
    worker_count: usize,
    iterations: usize,
    tier: PricingTier,
) -> CostEstimate {
    let active_minutes = duration_secs as f64 / 60.0;
    let tokens_per_minute = 1000.0;
    let iteration_multiplier = 1.0 + (iterations as f64 * 0.3);

    let total_tokens =
        (active_minutes * tokens_per_minute * worker_count as f64 * iteration_multiplier)
            .max(500.0);

    let input_tokens = (total_tokens * 0.75) as u64;
    let output_tokens = (total_tokens * 0.25) as u64;

    // Assume input is ~1/4 the cost of output for simplicity
    let input_cost = (input_tokens as f64 / 1_000_000.0) * (tier.dollars_per_1m_tokens() / 4.0);
    let output_cost = (output_tokens as f64 / 1_000_000.0) * tier.dollars_per_1m_tokens();
    let estimated_usd = input_cost + output_cost;

    CostEstimate {
        input_tokens,
        output_tokens,
        duration_secs,
        worker_count,
        estimated_usd,
        tier,
    }
}

/// Quick estimate for a team session.
pub fn estimate_team_cost(duration_secs: u64, worker_count: usize, role: &str) -> CostEstimate {
    let tier = if role.contains("architect") || role.contains("security") {
        PricingTier::Premium
    } else {
        PricingTier::Standard
    };
    estimate_cost(duration_secs, worker_count, 1, tier)
}

/// Quick estimate for an autopilot session.
pub fn estimate_autopilot_cost(duration_secs: u64, phases_completed: usize) -> CostEstimate {
    estimate_cost(duration_secs, 1, phases_completed, PricingTier::Standard)
}

/// Quick estimate for a Ralph session.
pub fn estimate_ralph_cost(
    duration_secs: u64,
    iterations: usize,
    stories_total: usize,
) -> CostEstimate {
    estimate_cost(
        duration_secs,
        1,
        iterations + stories_total,
        PricingTier::Standard,
    )
}
