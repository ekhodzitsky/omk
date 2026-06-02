use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::wire::{Event, PromptResult, PromptStatus, RawWireMessage};

use super::*;

#[tokio::test]
async fn test_mock_llm_client_complete() {
    let client = MockLlmClient::new(vec!["hello".to_string()]);
    let budget = TokenBudget::new(1000);

    let resp = client.complete("say hi", &budget).await.unwrap();
    assert_eq!(resp.content, "hello");
    assert_eq!(resp.model, "mock");

    let calls = client.take_calls().await;
    assert_eq!(calls, vec!["say hi"]);
}

#[tokio::test]
async fn test_mock_llm_client_complete_with_system() {
    let client = MockLlmClient::new(vec!["response".to_string()]);
    let budget = TokenBudget::new(1000);

    let resp = client
        .complete_with_system("You are helpful", "say hi", &budget)
        .await
        .unwrap();
    assert_eq!(resp.content, "response");

    let calls = client.take_calls().await;
    assert_eq!(calls.len(), 1);
    assert!(calls[0].contains("SYSTEM:"));
    assert!(calls[0].contains("You are helpful"));
}

#[tokio::test]
async fn test_mock_llm_client_exhausted_queue() {
    let client = MockLlmClient::new(vec![]);
    let budget = TokenBudget::new(1000);

    let result = client.complete("say hi", &budget).await;
    assert!(matches!(result, Err(LlmError::TransientNetwork(_))));
}

#[tokio::test]
async fn test_mock_llm_client_push_response() {
    let client = MockLlmClient::new(vec!["first".to_string()]);
    client.push_response("second".to_string()).await;

    let budget = TokenBudget::new(1000);
    let r1 = client.complete("a", &budget).await.unwrap();
    let r2 = client.complete("b", &budget).await.unwrap();
    assert_eq!(r1.content, "first");
    assert_eq!(r2.content, "second");
}

#[tokio::test]
async fn test_wire_llm_client_budget_exceeded() {
    let wire = Arc::new(Mutex::new(crate::wire::InMemoryWireClient::new()));
    let config = LlmClientConfig {
        model: "gpt-4".to_string(),
        max_tokens: 100,
        temperature: 0.0,
        timeout: Duration::from_secs(5),
        retry_policy: RetryPolicy::default(),
    };
    let client = WireLlmClient::new(wire, config, CostEstimator::new());
    let budget = TokenBudget::new(1);

    let result = client
        .complete("hello world this is a long prompt", &budget)
        .await;
    assert!(matches!(result, Err(LlmError::BudgetExhausted { .. })));
}

#[tokio::test]
async fn test_wire_llm_client_context_length_exceeded() {
    let wire = Arc::new(Mutex::new(crate::wire::InMemoryWireClient::new()));
    let config = LlmClientConfig {
        model: "gpt-4".to_string(),
        max_tokens: 1,
        temperature: 0.0,
        timeout: Duration::from_secs(5),
        retry_policy: RetryPolicy::default(),
    };
    let client = WireLlmClient::new(wire, config, CostEstimator::new());
    let budget = TokenBudget::new(10_000);

    let result = client.complete("hello world", &budget).await;
    assert!(matches!(
        result,
        Err(LlmError::ContextLengthExceeded { .. })
    ));
}

#[tokio::test]
async fn test_wire_llm_client_complete_success() {
    let wire = Arc::new(Mutex::new(crate::wire::InMemoryWireClient::new()));
    let config = LlmClientConfig {
        model: "gpt-4".to_string(),
        max_tokens: 1000,
        temperature: 0.0,
        timeout: Duration::from_secs(5),
        retry_policy: RetryPolicy {
            max_retries: 0,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        },
    };
    let client = WireLlmClient::new(wire.clone(), config, CostEstimator::new());

    let content_event = Event::ContentPart(crate::wire::ContentPart::Text(crate::wire::TextPart {
        text: "hello from wire".to_string(),
    }));
    let status_event = Event::StatusUpdate(crate::wire::StatusUpdate {
        context_usage: None,
        context_tokens: None,
        max_context_tokens: None,
        token_usage: Some(crate::wire::TokenUsage {
            input_other: 0,
            output: 5,
            input_cache_read: 0,
            input_cache_creation: 0,
        }),
        message_id: None,
        plan_mode: None,
    });

    wire.lock()
        .await
        .inject(RawWireMessage {
            jsonrpc: crate::wire::JsonRpcVersion::V2,
            id: None,
            method: Some("event".to_string()),
            params: Some(serde_json::to_value(&content_event).unwrap()),
            result: None,
            error: None,
        })
        .await;

    wire.lock()
        .await
        .inject(RawWireMessage {
            jsonrpc: crate::wire::JsonRpcVersion::V2,
            id: None,
            method: Some("event".to_string()),
            params: Some(serde_json::to_value(&status_event).unwrap()),
            result: None,
            error: None,
        })
        .await;

    wire.lock()
        .await
        .inject(RawWireMessage {
            jsonrpc: crate::wire::JsonRpcVersion::V2,
            id: Some("req-1".to_string()),
            method: None,
            params: None,
            result: Some(
                serde_json::to_value(PromptResult {
                    status: PromptStatus::Finished,
                    steps: None,
                })
                .unwrap(),
            ),
            error: None,
        })
        .await;

    let result = client
        .complete("say hi", &TokenBudget::new(1000))
        .await
        .unwrap();
    assert_eq!(result.content, "hello from wire");
    assert_eq!(result.completion_tokens, 5);
    assert_eq!(result.model, "gpt-4");
}

#[tokio::test]
async fn test_wire_llm_client_complete_with_system() {
    let wire = Arc::new(Mutex::new(crate::wire::InMemoryWireClient::new()));
    let config = LlmClientConfig {
        model: "gpt-4".to_string(),
        max_tokens: 1000,
        temperature: 0.0,
        timeout: Duration::from_secs(5),
        retry_policy: RetryPolicy {
            max_retries: 0,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        },
    };
    let client = WireLlmClient::new(wire.clone(), config, CostEstimator::new());

    wire.lock()
        .await
        .inject(RawWireMessage {
            jsonrpc: crate::wire::JsonRpcVersion::V2,
            id: Some("req-1".to_string()),
            method: None,
            params: None,
            result: Some(
                serde_json::to_value(PromptResult {
                    status: PromptStatus::Finished,
                    steps: None,
                })
                .unwrap(),
            ),
            error: None,
        })
        .await;

    let result = client
        .complete_with_system("You are helpful", "say hi", &TokenBudget::new(1000))
        .await
        .unwrap();

    // No ContentPart event was injected, so content is empty.
    assert_eq!(result.content, "");
    assert_eq!(result.model, "gpt-4");
}
