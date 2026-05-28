use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::runtime::classifier::{ClassifierInput, ClassifierOutput};
use crate::runtime::goal::chat_api::{ChildGoalEvent, ChildGoalHandle, CreateChildRequest};

#[async_trait]
pub trait ClassifierBackend: Send + Sync {
    async fn classify(&self, input: ClassifierInput) -> ClassifierOutput;
}

#[async_trait]
pub trait LlmDirectBackend: Send + Sync {
    async fn answer_direct(
        &self,
        prompt: &str,
        context: &[crate::runtime::classifier::ConversationTurn],
    ) -> anyhow::Result<u32>;
}

#[derive(Debug)]
pub struct SmallEditResult {
    pub worker_id: String,
    pub files_touched: u32,
    pub diff_summary: String,
}

#[derive(Debug)]
pub struct MediumPlanResult {
    pub workers: Vec<String>,
    pub steps_completed: u32,
    pub steps_failed: u32,
}

#[async_trait]
pub trait WireWorkerBackend: Send + Sync {
    async fn run_small_edit(&self, task: &str) -> anyhow::Result<SmallEditResult>;
    async fn run_medium_plan(&self, plan: &[String]) -> anyhow::Result<MediumPlanResult>;
}

#[async_trait]
pub trait GoalBridgeBackend: Send + Sync {
    async fn create_child(&self, req: CreateChildRequest) -> anyhow::Result<ChildGoalHandle>;
    async fn subscribe(&self, goal_id: &str)
        -> anyhow::Result<broadcast::Receiver<ChildGoalEvent>>;
}

// Production wrappers

pub struct ProductionClassifierBackend {
    pub inner: Arc<dyn crate::runtime::classifier::LlmClassifierBackend>,
    pub cache: tokio::sync::Mutex<lru::LruCache<u64, ClassifierOutput>>,
}

impl std::fmt::Debug for ProductionClassifierBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionClassifierBackend")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ClassifierBackend for ProductionClassifierBackend {
    async fn classify(&self, input: ClassifierInput) -> ClassifierOutput {
        crate::runtime::classifier::classify(input, self.inner.as_ref(), &self.cache).await
    }
}

pub struct ProductionLlmDirectBackend<W: crate::llm::LlmClient> {
    pub inner: Arc<tokio::sync::Mutex<W>>,
}

impl<W: crate::llm::LlmClient> std::fmt::Debug for ProductionLlmDirectBackend<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionLlmDirectBackend")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<W: crate::llm::LlmClient + Send + Sync> LlmDirectBackend for ProductionLlmDirectBackend<W> {
    async fn answer_direct(
        &self,
        prompt: &str,
        _context: &[crate::runtime::classifier::ConversationTurn],
    ) -> anyhow::Result<u32> {
        let budget = crate::llm::types::TokenBudget::new(usize::MAX);
        let resp = {
            let client = self.inner.lock().await;
            client.complete(prompt, &budget).await
        }?;
        drop(resp);
        Ok(0)
    }
}

#[derive(Debug)]
pub struct ProductionGoalBridgeBackend;

#[async_trait]
impl GoalBridgeBackend for ProductionGoalBridgeBackend {
    async fn create_child(&self, req: CreateChildRequest) -> anyhow::Result<ChildGoalHandle> {
        crate::runtime::goal::chat_api::create_child(req).await
    }

    async fn subscribe(&self, id: &str) -> anyhow::Result<broadcast::Receiver<ChildGoalEvent>> {
        crate::runtime::goal::chat_api::subscribe(id)
    }
}
