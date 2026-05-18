use serde::{Deserialize, Serialize};

/// Pricing tiers for heuristic cost estimation.
/// These are approximate rates based on typical API pricing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
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
///
/// This function is pure: no I/O, no panics for any input.
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

/// Build a [`CostEstimate`] from exact token counts.
///
/// Bypasses the heuristic duration-based estimator and uses real token
/// counts when they are already known (e.g. after calling
/// [`crate::cost::tokens::count_tokens`]).
pub fn estimate_from_exact_tokens(
    input_tokens: u64,
    output_tokens: u64,
    tier: PricingTier,
) -> CostEstimate {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * (tier.dollars_per_1m_tokens() / 4.0);
    let output_cost = (output_tokens as f64 / 1_000_000.0) * tier.dollars_per_1m_tokens();
    CostEstimate {
        input_tokens,
        output_tokens,
        duration_secs: 0,
        worker_count: 1,
        estimated_usd: input_cost + output_cost,
        tier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost_zero_duration() {
        let est = estimate_cost(0, 1, 0, PricingTier::Standard);
        assert_eq!(est.duration_secs, 0);
        assert_eq!(est.worker_count, 1);
        // Minimum 500 tokens enforced
        assert_eq!(est.input_tokens + est.output_tokens, 500);
        assert!(est.estimated_usd > 0.0);
    }

    #[test]
    fn test_estimate_cost_basic() {
        let est = estimate_cost(60, 2, 1, PricingTier::Standard);
        assert_eq!(est.duration_secs, 60);
        assert_eq!(est.worker_count, 2);
        assert_eq!(est.tier, PricingTier::Standard);
        // 1 min * 1000 tpm * 2 workers * 1.3 iterations = 2600 tokens
        assert_eq!(est.input_tokens + est.output_tokens, 2600);
    }

    #[test]
    fn test_estimate_cost_premium_tier() {
        let budget = estimate_cost(60, 1, 0, PricingTier::Budget);
        let standard = estimate_cost(60, 1, 0, PricingTier::Standard);
        let premium = estimate_cost(60, 1, 0, PricingTier::Premium);

        assert!(budget.estimated_usd < standard.estimated_usd);
        assert!(standard.estimated_usd < premium.estimated_usd);
    }

    #[test]
    fn test_pricing_tier_from_model_hint() {
        assert_eq!(PricingTier::from_model_hint("kimi"), PricingTier::Standard);
        assert_eq!(PricingTier::from_model_hint("gpt-4o"), PricingTier::Premium);
        assert_eq!(PricingTier::from_model_hint("haiku"), PricingTier::Budget);
        assert_eq!(
            PricingTier::from_model_hint("UNKNOWN"),
            PricingTier::Standard
        );
    }

    #[test]
    fn test_cost_estimate_formatted() {
        let est = CostEstimate {
            input_tokens: 3000,
            output_tokens: 1000,
            duration_secs: 120,
            worker_count: 3,
            estimated_usd: 1.5,
            tier: PricingTier::Standard,
        };
        let s = est.formatted();
        assert!(s.contains("~$1.5000"));
        assert!(s.contains("3 workers"));
        assert!(s.contains("~120s"));
        assert!(s.contains("~4K tokens"));
    }

    #[test]
    fn test_estimate_team_cost_premium_role() {
        let est = estimate_team_cost(60, 1, "security-audit");
        assert_eq!(est.tier, PricingTier::Premium);
    }

    #[test]
    fn test_estimate_team_cost_standard_role() {
        let est = estimate_team_cost(60, 1, "developer");
        assert_eq!(est.tier, PricingTier::Standard);
    }

    #[test]
    fn test_estimate_from_exact_tokens() {
        let est = estimate_from_exact_tokens(1_000_000, 0, PricingTier::Standard);
        assert_eq!(est.input_tokens, 1_000_000);
        assert_eq!(est.output_tokens, 0);
        assert_eq!(est.worker_count, 1);
        assert_eq!(est.duration_secs, 0);
        // input is 1/4 the output rate: 8.0 / 4 = 2.0
        assert!((est.estimated_usd - 2.0).abs() < f64::EPSILON);

        let est2 = estimate_from_exact_tokens(0, 1_000_000, PricingTier::Standard);
        assert!((est2.estimated_usd - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_from_exact_tokens_premium() {
        let est = estimate_from_exact_tokens(500_000, 500_000, PricingTier::Premium);
        // input: 500k @ $6/m = $3.0, output: 500k @ $24/m = $12.0
        assert!((est.estimated_usd - 15.0).abs() < 0.0001);
    }
}
