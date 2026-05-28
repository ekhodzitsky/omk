use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::llm::client::LlmClient;
use crate::llm::types::TokenBudget;

use super::types::ClassifierInput;

#[derive(Debug, Clone)]
pub struct RawLlmClassification {
    pub raw_json: String,
    pub model: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
}

#[async_trait]
pub trait LlmClassifierBackend: Send + Sync {
    async fn classify_llm(
        &self,
        input: &ClassifierInput,
    ) -> Result<RawLlmClassification, anyhow::Error>;
}

#[derive(Debug)]
pub struct WireLlmClassifierBackend<C> {
    client: Arc<C>,
}

impl<C> WireLlmClassifierBackend<C>
where
    C: LlmClient + Send + 'static,
{
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl<C> LlmClassifierBackend for WireLlmClassifierBackend<C>
where
    C: LlmClient + Send,
{
    async fn classify_llm(
        &self,
        input: &ClassifierInput,
    ) -> Result<RawLlmClassification, anyhow::Error> {
        let system = super::system_prompt::CLASSIFIER_SYSTEM_PROMPT;
        let budget = TokenBudget::new(4096);
        let user_prompt = build_user_prompt(input);
        let response = self
            .client
            .complete_with_system(system, &user_prompt, &budget)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(RawLlmClassification {
            raw_json: response.content,
            model: response.model,
            tokens_in: response.prompt_tokens as u32,
            tokens_out: response.completion_tokens as u32,
        })
    }
}

fn build_user_prompt(input: &ClassifierInput) -> String {
    let mut prompt = format!("User prompt:\n{}\n", input.prompt);
    if !input.recent_conversation.is_empty() {
        prompt.push_str("\nRecent conversation:\n");
        for turn in &input.recent_conversation {
            let role = match turn.role {
                super::types::Role::User => "User",
                super::types::Role::Assistant => "Assistant",
            };
            prompt.push_str(&format!("{}: {}\n", role, turn.text));
        }
    }
    prompt
}

#[derive(Debug)]
pub struct MockLlmClassifier {
    answers: HashMap<u64, RawLlmClassification>,
}

impl Default for MockLlmClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLlmClassifier {
    pub fn new() -> Self {
        Self {
            answers: HashMap::new(),
        }
    }

    pub fn with_answer(mut self, prompt_hash: u64, raw: RawLlmClassification) -> Self {
        self.answers.insert(prompt_hash, raw);
        self
    }
}

#[async_trait]
impl LlmClassifierBackend for MockLlmClassifier {
    async fn classify_llm(
        &self,
        input: &ClassifierInput,
    ) -> Result<RawLlmClassification, anyhow::Error> {
        let key = super::cache::cache_key(&input.prompt);
        self.answers
            .get(&key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("mock answer not found for prompt hash {}", key))
    }
}
