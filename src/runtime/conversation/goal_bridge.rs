use crate::runtime::goal::chat_api::{ChildGoalEvent, ChildGoalHandle};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Trait exposed so W3 (conversation owner) can glue this to their
/// `EventBus` once their workstream lands.  Until then this is a
/// stable contract across workstream boundaries.
pub trait EngineEventSink: Send + Sync {
    fn publish_child_event(&self, goal_id: &str, ev: ChildGoalEvent);
}

/// Bridges child-goal events into the engine event bus.
pub struct GoalBridge {
    sink: Arc<dyn EngineEventSink>,
}

impl GoalBridge {
    pub fn new(sink: Arc<dyn EngineEventSink>) -> Self {
        Self { sink }
    }

    /// Attach a consumer task that forwards all events from `child`
    /// into the configured `EngineEventSink`.
    pub async fn attach(&self, child: &ChildGoalHandle) -> tokio::task::JoinHandle<()> {
        let goal_id = child.goal_id.clone();
        let sink = self.sink.clone();
        let mut rx = crate::runtime::goal::chat_api::subscribe(&goal_id)
            .expect("attach to existing child goal");
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                sink.publish_child_event(&goal_id, ev);
            }
        })
    }
}
