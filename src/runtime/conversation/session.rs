use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug)]
pub struct SessionCtx {
    pub session_id: String,
    pub project_root: PathBuf,
    pub started_at: Instant,
    pub is_first_prompt: AtomicBool,
    pub cost_soft_warned: AtomicBool,
    pub cumulative_cost_usd: tokio::sync::Mutex<f32>,
    pub active_large_goal: tokio::sync::Mutex<Option<String>>,
    pub active_medium_goals: tokio::sync::Mutex<Vec<String>>,
    pub active_small_workers: tokio::sync::Mutex<Vec<String>>,
}

impl SessionCtx {
    pub fn new(session_id: String, project_root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            session_id,
            project_root,
            started_at: Instant::now(),
            is_first_prompt: AtomicBool::new(true),
            cost_soft_warned: AtomicBool::new(false),
            cumulative_cost_usd: tokio::sync::Mutex::new(0.0),
            active_large_goal: tokio::sync::Mutex::new(None),
            active_medium_goals: tokio::sync::Mutex::new(Vec::new()),
            active_small_workers: tokio::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn first_prompt_done(&self) {
        self.is_first_prompt.store(false, Ordering::Release);
    }

    pub fn is_first(&self) -> bool {
        self.is_first_prompt.load(Ordering::Acquire)
    }
}
