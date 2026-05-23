use std::collections::HashMap;

use super::error::LlmError;

/// Estimates token counts and USD costs for LLM calls.
#[derive(Debug, Clone)]
pub struct CostEstimator {
    model_rates: HashMap<String, f64>,
}

impl Default for CostEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl CostEstimator {
    /// Create a new estimator with built-in rates for common models.
    pub fn new() -> Self {
        let mut model_rates = HashMap::new();
        // Rates are dollars per 1K tokens (input + output averaged).
        model_rates.insert("gpt-4".to_string(), 0.03);
        model_rates.insert("gpt-4o".to_string(), 0.005);
        model_rates.insert("gpt-3.5-turbo".to_string(), 0.002);
        model_rates.insert("kimi-k2".to_string(), 0.008);
        model_rates.insert("kimi-k1.5".to_string(), 0.008);
        model_rates.insert("claude-3-opus".to_string(), 0.015);
        model_rates.insert("claude-3-sonnet".to_string(), 0.003);
        model_rates.insert("default".to_string(), 0.005);
        Self { model_rates }
    }

    /// Estimate cost in USD for a request/response pair.
    pub fn estimate(&self, prompt_tokens: usize, completion_tokens: usize, model: &str) -> f64 {
        let rate = self
            .model_rates
            .get(model)
            .or_else(|| self.model_rates.get("default"))
            .copied()
            .unwrap_or(0.005);
        let total = prompt_tokens + completion_tokens;
        (total as f64 / 1_000.0) * rate
    }

    /// Count tokens in a text string using tiktoken-rs.
    ///
    /// Falls back to `cl100k_base` for unknown models.
    pub fn count_tokens(&self, text: &str, model: &str) -> Result<usize, LlmError> {
        let lower = model.to_lowercase();
        let bpe = if lower.contains("gpt-4")
            || lower.contains("gpt-3.5-turbo")
            || lower.contains("kimi")
            || lower.contains("text-embedding")
            || lower.contains("claude")
        {
            tiktoken_rs::cl100k_base_singleton()
        } else if lower.contains("text-davinci-003")
            || lower.contains("text-davinci-002")
            || lower.contains("code-davinci")
            || lower.contains("code-cushman")
        {
            tiktoken_rs::p50k_base_singleton()
        } else if lower.contains("text-davinci-001")
            || lower.contains("davinci")
            || lower.contains("curie")
            || lower.contains("babbage")
            || lower.contains("ada")
            || lower.contains("gpt2")
        {
            tiktoken_rs::r50k_base_singleton()
        } else {
            tiktoken_rs::cl100k_base_singleton()
        };

        let tokens = bpe.encode_with_special_tokens(text);
        Ok(tokens.len())
    }

    /// Register a custom rate for a model (dollars per 1K tokens).
    pub fn set_rate(&mut self, model: String, rate: f64) {
        self.model_rates.insert(model, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_estimator_count_tokens() {
        let estimator = CostEstimator::new();
        let n = estimator.count_tokens("hello world", "gpt-4").unwrap();
        // "hello world" is 2 tokens in cl100k_base
        assert_eq!(n, 2);
    }

    #[test]
    fn test_cost_estimator_count_tokens_empty() {
        let estimator = CostEstimator::new();
        let n = estimator.count_tokens("", "gpt-4").unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_cost_estimator_estimate_usd() {
        let estimator = CostEstimator::new();
        let usd = estimator.estimate(1_000, 1_000, "gpt-4");
        // 2000 tokens at $0.03 per 1K = $0.06
        assert!((usd - 0.06).abs() < f64::EPSILON * 10.0);
    }

    #[test]
    fn test_cost_estimator_estimate_unknown_model() {
        let estimator = CostEstimator::new();
        let usd = estimator.estimate(1_000, 0, "unknown-model");
        // Falls back to default rate (0.005 per 1K)
        assert!((usd - 0.005).abs() < f64::EPSILON * 10.0);
    }

    #[test]
    fn test_cost_estimator_custom_rate() {
        let mut estimator = CostEstimator::new();
        estimator.set_rate("custom".to_string(), 0.01);
        let usd = estimator.estimate(1_000, 0, "custom");
        assert!((usd - 0.01).abs() < f64::EPSILON * 10.0);
    }

    #[test]
    fn test_cost_estimator_count_tokens_davinci() {
        let estimator = CostEstimator::new();
        let n = estimator
            .count_tokens("hello world", "text-davinci-003")
            .unwrap();
        assert!(n > 0);
    }

    #[test]
    fn test_cost_estimator_count_tokens_unknown_model() {
        let estimator = CostEstimator::new();
        let n = estimator
            .count_tokens("hello world", "unknown-model")
            .unwrap();
        // Falls back to cl100k_base, same as gpt-4
        let gpt4 = estimator.count_tokens("hello world", "gpt-4").unwrap();
        assert_eq!(n, gpt4);
    }

    #[test]
    fn test_cost_estimator_default() {
        let estimator: CostEstimator = Default::default();
        let usd = estimator.estimate(1_000, 0, "gpt-4");
        assert!(usd > 0.0);
    }
}
