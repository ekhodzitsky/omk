use std::sync::Arc;

use crate::runtime::goal::chat_api::{ChildGoalEvent, ChildGoalHandle};
use tokio_util::sync::CancellationToken;

/// Trait exposed so W3 (conversation owner) can glue this to their
/// `EventBus` once their workstream lands.  Until then this is a
/// stable contract across workstream boundaries.
pub trait EngineEventSink: Send + Sync {
    fn publish_child_event(&self, goal_id: &str, ev: ChildGoalEvent);
}

/// Bridges child-goal events into the engine event bus.
pub struct GoalBridge {
    sink: Arc<dyn EngineEventSink>,
    cancel: CancellationToken,
}

impl std::fmt::Debug for GoalBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalBridge").finish_non_exhaustive()
    }
}

impl GoalBridge {
    pub fn new(sink: Arc<dyn EngineEventSink>) -> Self {
        Self {
            sink,
            cancel: CancellationToken::new(),
        }
    }

    /// Attach a consumer task that forwards all events from `child`
    /// into the configured `EngineEventSink`.
    ///
    /// The spawned task is tied to the bridge's `CancellationToken`;
    /// dropping the bridge aborts the task.
    pub fn attach(&self, child: &ChildGoalHandle) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let goal_id = child.goal_id.clone();
        let sink = self.sink.clone();
        let mut rx = crate::runtime::goal::chat_api::subscribe(&goal_id)?;
        let child_cancel = self.cancel.child_token();
        Ok(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = child_cancel.cancelled() => break,
                    maybe_ev = rx.recv() => {
                        match maybe_ev {
                            Ok(ev) => sink.publish_child_event(&goal_id, ev),
                            Err(_) => break,
                        }
                    }
                }
            }
        }))
    }
}

impl Drop for GoalBridge {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
