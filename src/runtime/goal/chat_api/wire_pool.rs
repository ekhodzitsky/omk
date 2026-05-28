use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::wire::client::{ProcessWireClient, WireClient};

/// Factory for creating wire clients. Production default spawns
/// `ProcessWireClient` processes; tests can inject mocks.
#[allow(async_fn_in_trait)]
pub trait WireClientFactory: Send + Sync + 'static {
    type Client: WireClient + Send + 'static;
    async fn create(&self) -> anyhow::Result<Self::Client>;
}

/// Production factory that spawns real Kimi wire processes.
#[derive(Debug, Clone, Copy)]
pub struct ProcessWireClientFactory;

impl WireClientFactory for ProcessWireClientFactory {
    type Client = ProcessWireClient;

    async fn create(&self) -> anyhow::Result<Self::Client> {
        ProcessWireClient::spawn("kimi", None, None, None).await
    }
}

pub struct WirePool<F: WireClientFactory = ProcessWireClientFactory> {
    size: usize,
    idle: Mutex<VecDeque<PooledWorker<F::Client>>>,
    in_use: Mutex<HashSet<String>>,
    idle_ttl: Duration,
    factory: F,
    cancel: CancellationToken,
}

impl<F: WireClientFactory> std::fmt::Debug for WirePool<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WirePool")
            .field("size", &self.size)
            .field("idle_ttl", &self.idle_ttl)
            .finish_non_exhaustive()
    }
}

pub struct PooledWorker<C> {
    pub inner: C,
    pub id: String,
    pub acquired_at: Instant,
}

impl<C> std::fmt::Debug for PooledWorker<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledWorker")
            .field("id", &self.id)
            .field("acquired_at", &self.acquired_at)
            .finish_non_exhaustive()
    }
}

impl WirePool {
    pub fn new(size: usize) -> Self {
        Self::with_factory(size, ProcessWireClientFactory)
    }
}

impl<F: WireClientFactory + 'static> WirePool<F> {
    pub fn with_factory(size: usize, factory: F) -> Self {
        Self {
            size,
            idle: Default::default(),
            in_use: Default::default(),
            idle_ttl: Duration::from_secs(5 * 60),
            factory,
            cancel: CancellationToken::new(),
        }
    }

    pub async fn acquire(&self) -> anyhow::Result<PooledWorker<F::Client>> {
        let mut idle = self.idle.lock().await;

        while let Some(entry) = idle.pop_front() {
            if entry.acquired_at.elapsed() < self.idle_ttl {
                let mut in_use = self.in_use.lock().await;
                in_use.insert(entry.id.clone());
                return Ok(entry);
            }
            // Expired entry — Drop handles partial cleanup; full shutdown
            // is not required here since the client is already stale.
        }

        drop(idle);

        // Spillover: always spawn a fresh worker even if size exceeded.
        let id = format!("wire-{}", uuid::Uuid::new_v4());
        let mut in_use = self.in_use.lock().await;
        in_use.insert(id.clone());
        drop(in_use);

        let inner = self.factory.create().await?;

        Ok(PooledWorker {
            inner,
            id,
            acquired_at: Instant::now(),
        })
    }

    pub async fn release(&self, w: PooledWorker<F::Client>) {
        let mut in_use = self.in_use.lock().await;
        in_use.remove(&w.id);
        let current_in_use = in_use.len();
        let should_pool = current_in_use < self.size;
        drop(in_use);

        if should_pool {
            let mut idle = self.idle.lock().await;
            idle.push_back(PooledWorker {
                inner: w.inner,
                id: w.id,
                acquired_at: Instant::now(),
            });
        } else {
            if let Err(e) = w.inner.shutdown().await {
                warn!(worker_id = %w.id, error = %e, "Failed to shutdown wire pool worker");
            }
        }
    }

    pub fn spawn_idle_eviction_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let cancel = self.cancel.child_token();
        tokio::spawn(async move {
            let interval = Duration::from_secs(60);
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {}
                    _ = cancel.cancelled() => break,
                }
                let mut idle = self.idle.lock().await;
                idle.retain(|w| w.acquired_at.elapsed() < self.idle_ttl);
            }
        })
    }
}
