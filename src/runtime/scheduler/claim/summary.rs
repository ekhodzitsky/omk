#[derive(Debug, Clone, Default)]
pub struct TaskSummary {
    pub pending: usize,
    pub claimed: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

impl TaskSummary {
    pub fn total(&self) -> usize {
        self.pending + self.claimed + self.running + self.completed + self.failed + self.cancelled
    }

    pub fn done(&self) -> usize {
        self.completed + self.failed + self.cancelled
    }
}
