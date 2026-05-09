use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Lifecycle state of a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerState {
    /// Worker is starting up and not yet ready.
    Starting,
    /// Worker is idle and available to claim tasks.
    Ready,
    /// Worker is actively executing a task.
    Busy,
    /// Worker heartbeat is stale or unresponsive.
    Stalled,
    /// Worker is unrecoverable.
    Dead,
    /// Worker has been explicitly stopped.
    Stopped,
}

impl WorkerState {
    /// Returns true if the worker is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, WorkerState::Dead | WorkerState::Stopped)
    }

    /// Returns true if the worker can claim new tasks.
    pub fn is_available(&self) -> bool {
        matches!(self, WorkerState::Ready)
    }
}

/// In-memory registry of worker states.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerStateMap {
    states: HashMap<String, WorkerState>,
}

impl WorkerStateMap {
    /// Get the current state of a worker, defaulting to `Starting`.
    pub fn get(&self, worker_id: &str) -> WorkerState {
        self.states
            .get(worker_id)
            .copied()
            .unwrap_or(WorkerState::Starting)
    }

    /// Set the state of a worker, returning the previous state if any.
    pub fn set(&mut self, worker_id: &str, state: WorkerState) -> Option<WorkerState> {
        self.states.insert(worker_id.to_string(), state)
    }

    /// Transition a worker to a new state if it differs from the current state.
    /// Returns the previous state if a transition occurred.
    pub fn transition(&mut self, worker_id: &str, new_state: WorkerState) -> Option<WorkerState> {
        let old = self.get(worker_id);
        if old != new_state {
            self.set(worker_id, new_state);
            Some(old)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_state_terminal() {
        assert!(WorkerState::Dead.is_terminal());
        assert!(WorkerState::Stopped.is_terminal());
        assert!(!WorkerState::Starting.is_terminal());
        assert!(!WorkerState::Ready.is_terminal());
        assert!(!WorkerState::Busy.is_terminal());
        assert!(!WorkerState::Stalled.is_terminal());
    }

    #[test]
    fn worker_state_available() {
        assert!(WorkerState::Ready.is_available());
        assert!(!WorkerState::Starting.is_available());
        assert!(!WorkerState::Busy.is_available());
        assert!(!WorkerState::Stalled.is_available());
        assert!(!WorkerState::Dead.is_available());
        assert!(!WorkerState::Stopped.is_available());
    }

    #[test]
    fn worker_state_map_transition() {
        let mut map = WorkerStateMap::default();
        assert_eq!(map.get("w1"), WorkerState::Starting);

        let old = map.transition("w1", WorkerState::Ready);
        assert_eq!(old, Some(WorkerState::Starting));
        assert_eq!(map.get("w1"), WorkerState::Ready);

        // No transition when state is the same
        let old = map.transition("w1", WorkerState::Ready);
        assert_eq!(old, None);
    }
}
