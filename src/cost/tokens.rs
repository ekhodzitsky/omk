use anyhow::{Context, Result};

use crate::wire::protocol::Request;

/// Return the tiktoken-rs encoding matching the model name.
///
/// Supported mappings:
/// - `cl100k_base`: GPT-4, GPT-4o, Kimi default, text-embedding-ada-002, etc.
/// - `p50k_base`: Code models, text-davinci-002, text-davinci-003.
/// - `r50k_base`: GPT-3, text-davinci-001.
fn encoding_for_model(model: &str) -> Result<tiktoken_rs::CoreBPE> {
    let lower = model.to_lowercase();
    // cl100k_base models
    if lower.contains("gpt-4")
        || lower.contains("gpt-3.5-turbo")
        || lower.contains("kimi")
        || lower.contains("text-embedding")
        || lower.contains("claude")
    {
        tiktoken_rs::cl100k_base()
    }
    // p50k_base models
    else if lower.contains("text-davinci-003")
        || lower.contains("text-davinci-002")
        || lower.contains("code-davinci")
        || lower.contains("code-cushman")
    {
        tiktoken_rs::p50k_base()
    }
    // r50k_base / gpt2 models
    else if lower.contains("text-davinci-001")
        || lower.contains("davinci")
        || lower.contains("curie")
        || lower.contains("babbage")
        || lower.contains("ada")
        || lower.contains("gpt2")
    {
        tiktoken_rs::r50k_base()
    } else {
        // Default to cl100k_base for unknown models (covers most modern usage)
        tiktoken_rs::cl100k_base()
    }
}

/// Exact token count for a plain text string.
///
/// `model` is a free-form hint such as `"gpt-4"`, `"kimi-k2"`, or
/// `"text-davinci-003"`.  Unknown models fall back to `cl100k_base`.
pub fn count_tokens(text: &str, model: &str) -> Result<usize> {
    let bpe = encoding_for_model(model)?;
    let tokens = bpe.encode_with_special_tokens(text);
    Ok(tokens.len())
}

/// Exact token count for a wire [`Request`] message.
///
/// The request is serialized to JSON and the resulting string is tokenised.
pub fn count_message_tokens(message: &Request, model: &str) -> Result<usize> {
    let json = serde_json::to_string(message).context("serialize wire Request to JSON")?;
    count_tokens(&json, model)
}

/// Compute an estimated USD cost from an exact token count.
///
/// Convenience helper that bridges the token-counting module with the
/// estimator module.
pub fn estimated_usd_from_exact_tokens(
    input_tokens: usize,
    output_tokens: usize,
    tier: &super::estimator::PricingTier,
) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * (tier.dollars_per_1m_tokens() / 4.0);
    let output_cost = (output_tokens as f64 / 1_000_000.0) * tier.dollars_per_1m_tokens();
    input_cost + output_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_empty() {
        assert_eq!(count_tokens("", "gpt-4").unwrap(), 0);
    }

    #[test]
    fn test_count_tokens_known_text() {
        // "hello world" is 2 tokens in cl100k_base
        let n = count_tokens("hello world", "gpt-4").unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn test_count_tokens_different_models() {
        let text = "fn main() { println!(\"Hello\"); }";
        let gpt4 = count_tokens(text, "gpt-4").unwrap();
        let kimi = count_tokens(text, "kimi-k2").unwrap();
        let davinci = count_tokens(text, "text-davinci-003").unwrap();

        // gpt-4 and kimi both use cl100k_base
        assert_eq!(gpt4, kimi);
        // p50k_base usually produces a different count for code
        assert!(davinci > 0);
    }

    #[test]
    fn test_count_message_tokens_approval_request() {
        let req = Request::ApprovalRequest(crate::wire::protocol::ApprovalRequest {
            id: "a1".to_string(),
            tool_call_id: "tc1".to_string(),
            sender: "agent".to_string(),
            action: "write_file".to_string(),
            description: "write to /tmp/test".to_string(),
            display: None,
            source_kind: None,
            source_id: None,
            agent_id: None,
            subagent_type: None,
            source_description: None,
        });
        let n = count_message_tokens(&req, "gpt-4").unwrap();
        assert!(n > 0);
    }

    #[test]
    fn test_count_message_tokens_tool_call_request() {
        let req = Request::ToolCallRequest(crate::wire::protocol::ToolCallRequest {
            id: "t1".to_string(),
            name: "read_file".to_string(),
            arguments: Some("{\"path\":\"/tmp/test\"}".to_string()),
        });
        let n = count_message_tokens(&req, "gpt-4").unwrap();
        assert!(n > 0);
    }

    #[test]
    fn test_estimated_usd_from_exact_tokens() {
        let tier = super::super::estimator::PricingTier::Standard;
        let usd = estimated_usd_from_exact_tokens(1_000_000, 0, &tier);
        // input is 1/4 the output rate
        assert!((usd - 2.0).abs() < f64::EPSILON);

        let usd2 = estimated_usd_from_exact_tokens(0, 1_000_000, &tier);
        assert!((usd2 - 8.0).abs() < f64::EPSILON);
    }
}
