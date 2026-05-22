use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::runtime::classifier::{ClassifierInput, ClassifierOutput, ConversationTurn};
use crate::runtime::escalation::backends::{
    ClassifierBackend, GoalBridgeBackend, LlmDirectBackend, MediumPlanResult, SmallEditResult,
    WireWorkerBackend,
};
use crate::runtime::goal::chat_api::{ChildGoalEvent, ChildGoalHandle, CreateChildRequest};

#[derive(Debug)]
pub struct MockClassifier {
    pub output: ClassifierOutput,
}

impl MockClassifier {
    pub fn new(output: ClassifierOutput) -> Self {
        Self { output }
    }
}

#[async_trait]
impl ClassifierBackend for MockClassifier {
    async fn classify(&self, _input: ClassifierInput) -> ClassifierOutput {
        self.output.clone()
    }
}

#[derive(Debug)]
pub struct MockLlmDirect {
    pub latency_ms: u32,
}

impl MockLlmDirect {
    pub fn new(latency_ms: u32) -> Self {
        Self { latency_ms }
    }
}

#[async_trait]
impl LlmDirectBackend for MockLlmDirect {
    async fn answer_direct(
        &self,
        _prompt: &str,
        _context: &[ConversationTurn],
    ) -> anyhow::Result<u32> {
        Ok(self.latency_ms)
    }
}

pub struct MockWireWorker {
    pub small_result: SmallEditResult,
    pub medium_result: MediumPlanResult,
    pub small_blocks: tokio::sync::Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>,
    pub medium_blocks: tokio::sync::Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>,
}

impl std::fmt::Debug for MockWireWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockWireWorker")
            .field("small_result", &self.small_result)
            .field("medium_result", &self.medium_result)
            .finish_non_exhaustive()
    }
}

impl MockWireWorker {
    pub fn new(small_result: SmallEditResult, medium_result: MediumPlanResult) -> Self {
        Self {
            small_result,
            medium_result,
            small_blocks: tokio::sync::Mutex::new(Vec::new()),
            medium_blocks: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    pub async fn push_small_block(&self, rx: tokio::sync::oneshot::Receiver<()>) {
        self.small_blocks.lock().await.push(rx);
    }

    pub async fn push_medium_block(&self, rx: tokio::sync::oneshot::Receiver<()>) {
        self.medium_blocks.lock().await.push(rx);
    }
}

#[async_trait]
impl WireWorkerBackend for MockWireWorker {
    async fn run_small_edit(&self, _task: &str) -> anyhow::Result<SmallEditResult> {
        let rx = {
            let mut guard = self.small_blocks.lock().await;
            guard.pop()
        };
        if let Some(rx) = rx {
            let _ = rx.await;
        }
        Ok(SmallEditResult {
            worker_id: self.small_result.worker_id.clone(),
            files_touched: self.small_result.files_touched,
            diff_summary: self.small_result.diff_summary.clone(),
        })
    }

    async fn run_medium_plan(&self, _plan: &[String]) -> anyhow::Result<MediumPlanResult> {
        let rx = {
            let mut guard = self.medium_blocks.lock().await;
            guard.pop()
        };
        if let Some(rx) = rx {
            let _ = rx.await;
        }
        Ok(MediumPlanResult {
            workers: self.medium_result.workers.clone(),
            steps_completed: self.medium_result.steps_completed,
            steps_failed: self.medium_result.steps_failed,
        })
    }
}

pub struct MockGoalBridge {
    pub handle: ChildGoalHandle,
    pub created: tokio::sync::Mutex<Vec<CreateChildRequest>>,
}

impl std::fmt::Debug for MockGoalBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockGoalBridge")
            .field("created", &self.created.blocking_lock().len())
            .finish_non_exhaustive()
    }
}

impl MockGoalBridge {
    pub fn new(handle: ChildGoalHandle) -> Self {
        Self {
            handle,
            created: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl GoalBridgeBackend for MockGoalBridge {
    async fn create_child(&self, req: CreateChildRequest) -> anyhow::Result<ChildGoalHandle> {
        self.created.lock().await.push(req);
        Ok(ChildGoalHandle {
            goal_id: self.handle.goal_id.clone(),
            session_id: self.handle.session_id.clone(),
            created_at: self.handle.created_at,
        })
    }

    async fn subscribe(
        &self,
        _goal_id: &str,
    ) -> anyhow::Result<broadcast::Receiver<ChildGoalEvent>> {
        let (tx, rx) = broadcast::channel(4);
        let _ = tx;
        Ok(rx)
    }
}
