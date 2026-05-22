use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::events::ChildGoalEvent;

pub(crate) struct GoalEntry {
    pub(crate) sender: broadcast::Sender<ChildGoalEvent>,
    pub(crate) tail_task: JoinHandle<()>,
    pub(crate) exec_task: JoinHandle<()>,
}

static REGISTRY: Lazy<std::sync::RwLock<HashMap<String, Arc<GoalEntry>>>> =
    Lazy::new(|| std::sync::RwLock::new(HashMap::new()));

pub(crate) fn register(goal_id: String, entry: GoalEntry) -> broadcast::Sender<ChildGoalEvent> {
    let sender = entry.sender.clone();
    let mut map = REGISTRY.write().expect("registry lock poisoned");
    map.insert(goal_id, Arc::new(entry));
    sender
}

pub(crate) fn get(goal_id: &str) -> Option<Arc<GoalEntry>> {
    let map = REGISTRY.read().expect("registry lock poisoned");
    map.get(goal_id).cloned()
}

pub(crate) fn deregister(goal_id: &str) {
    let mut map = REGISTRY.write().expect("registry lock poisoned");
    if let Some(entry) = map.remove(goal_id) {
        entry.tail_task.abort();
        entry.exec_task.abort();
    }
}
