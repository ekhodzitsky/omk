use anyhow::Result;

use super::types::SessionCost;

#[allow(async_fn_in_trait)]
/// Trait for cost storage backends.
///
/// Implementations may be file-based, in-memory, or remote.
/// The trait is object-safe-free; consumers use generics (`CostTracker<S>`).
pub trait CostSink {
    /// Persist the full list of session costs.
    async fn save(&self, costs: &[SessionCost]) -> Result<()>;

    /// Load the full list of session costs.
    async fn load(&self) -> Result<Vec<SessionCost>>;
}

/// In-memory cost sink for unit tests.
///
/// Stores data in a `tokio::sync::Mutex<Vec<SessionCost>>` so it can be shared
/// across async tasks without blocking the executor.
pub struct InMemoryCostSink {
    inner: tokio::sync::Mutex<Vec<SessionCost>>,
}

impl Default for InMemoryCostSink {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryCostSink {
    pub fn new() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

impl CostSink for InMemoryCostSink {
    async fn save(&self, costs: &[SessionCost]) -> Result<()> {
        let mut guard = self.inner.lock().await;
        guard.clear();
        guard.extend_from_slice(costs);
        Ok(())
    }

    async fn load(&self) -> Result<Vec<SessionCost>> {
        let guard = self.inner.lock().await;
        Ok(guard.clone())
    }
}
