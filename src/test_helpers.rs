//! Test helpers for isolated XDG/HOME directory setup and reusable mocks.
//!
//! Intended for use in unit and integration tests to avoid polluting the user's
//! real home directory with test artifacts.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use tokio::sync::Mutex;

use crate::cost::{CostSink, SessionCost};
use crate::runtime::events::{Event, EventSink};
use crate::wire::client::{InMemoryWireClient, WireClient};
use crate::wire::protocol::{
    InitializeParams, InitializeResult, JsonRpcRequest, PromptResult, ReplayResult,
    SetPlanModeResult, SteerResult,
};

// ---------------------------------------------------------------------------
// Isolated XDG environment
// ---------------------------------------------------------------------------

/// Sets up isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`,
/// `XDG_DATA_HOME`, and `XDG_CACHE_HOME` inside a temporary directory.
///
/// Returns the temp directory handle (must be kept alive for the duration
/// of the test) and a vector of environment variable tuples.
pub fn isolated_xdg_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let xdg_config = home.join(".config");
    let xdg_state = home.join(".local").join("state");
    let xdg_data = home.join(".local").join("share");
    let xdg_cache = home.join(".cache");

    std::fs::create_dir_all(&xdg_config).unwrap();
    std::fs::create_dir_all(&xdg_state).unwrap();
    std::fs::create_dir_all(&xdg_data).unwrap();
    std::fs::create_dir_all(&xdg_cache).unwrap();

    let envs = vec![
        ("HOME", home.clone()),
        ("XDG_CONFIG_HOME", xdg_config),
        ("XDG_STATE_HOME", xdg_state),
        ("XDG_DATA_HOME", xdg_data),
        ("XDG_CACHE_HOME", xdg_cache),
    ];

    (tmp, envs)
}

// ---------------------------------------------------------------------------
// MockWireClient
// ---------------------------------------------------------------------------

/// In-memory mock implementation of [`WireClient`] for unit tests.
///
/// Wraps [`InMemoryWireClient`] and provides a convenient `drain()` method
/// for asserting on outgoing messages.
#[derive(Debug)]
pub struct MockWireClient {
    inner: InMemoryWireClient,
}

impl Default for MockWireClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWireClient {
    pub fn new() -> Self {
        Self {
            inner: InMemoryWireClient::new(),
        }
    }

    /// Inject an incoming message for the client to read.
    pub async fn inject(&self, msg: crate::wire::client::WireMessage) {
        self.inner.inject(msg).await;
    }

    /// Take all messages sent by the client.
    pub async fn drain(&self) -> Vec<serde_json::Value> {
        self.inner.outgoing().await
    }
}

#[allow(async_fn_in_trait)]
impl WireClient for MockWireClient {
    fn next_id(&mut self) -> String {
        self.inner.next_id()
    }

    async fn send_request<Params: serde::Serialize>(
        &mut self,
        req: &JsonRpcRequest<Params>,
    ) -> anyhow::Result<()> {
        self.inner.send_request(req).await
    }

    async fn read_message(&mut self) -> anyhow::Result<crate::wire::client::WireMessage> {
        self.inner.read_message().await
    }

    async fn read_message_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> anyhow::Result<crate::wire::client::WireMessage> {
        self.inner.read_message_timeout(timeout).await
    }

    async fn read_response<T: serde::de::DeserializeOwned>(
        &mut self,
        expected_id: &str,
    ) -> anyhow::Result<T> {
        self.inner.read_response(expected_id).await
    }

    async fn send_response<T: serde::Serialize>(
        &mut self,
        id: &str,
        result: T,
    ) -> anyhow::Result<()> {
        self.inner.send_response(id, result).await
    }

    async fn send_error(&mut self, id: &str, code: i32, message: &str) -> anyhow::Result<()> {
        self.inner.send_error(id, code, message).await
    }

    async fn initialize(&mut self, params: InitializeParams) -> anyhow::Result<InitializeResult> {
        self.inner.initialize(params).await
    }

    fn is_handshake_done(&self) -> bool {
        self.inner.is_handshake_done()
    }

    async fn shutdown(self) -> anyhow::Result<()> {
        self.inner.shutdown().await
    }

    async fn prompt(&mut self, user_input: &str) -> anyhow::Result<PromptResult> {
        self.inner.prompt(user_input).await
    }

    async fn start_prompt(&mut self, user_input: &str) -> anyhow::Result<String> {
        self.inner.start_prompt(user_input).await
    }

    async fn replay(&mut self) -> anyhow::Result<ReplayResult> {
        self.inner.replay().await
    }

    async fn steer(&mut self, user_input: &str) -> anyhow::Result<SteerResult> {
        self.inner.steer(user_input).await
    }

    async fn set_plan_mode(&mut self, enabled: bool) -> anyhow::Result<SetPlanModeResult> {
        self.inner.set_plan_mode(enabled).await
    }

    async fn cancel(&mut self) -> anyhow::Result<()> {
        self.inner.cancel().await
    }
}

// ---------------------------------------------------------------------------
// MockEventSink
// ---------------------------------------------------------------------------

/// In-memory mock implementation of [`EventSink`] for unit tests.
///
/// Records all sent events in a `Vec` so tests can assert on them.
#[derive(Debug)]
pub struct MockEventSink {
    records: Mutex<Vec<Event>>,
}

impl Default for MockEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEventSink {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    /// Take all recorded events, clearing the internal buffer.
    pub async fn take_records(&self) -> Vec<Event> {
        std::mem::take(&mut *self.records.lock().await)
    }

    /// Peek at recorded events without clearing.
    pub async fn records(&self) -> Vec<Event> {
        self.records.lock().await.clone()
    }
}

impl EventSink for MockEventSink {
    async fn send_event(&self, event: &Event) -> anyhow::Result<()> {
        self.records.lock().await.push(event.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MockCostSink
// ---------------------------------------------------------------------------

/// In-memory mock implementation of [`CostSink`] for unit tests.
///
/// Records all saved costs in a `Vec` so tests can assert on them.
#[derive(Debug)]
pub struct MockCostSink {
    records: Mutex<Vec<SessionCost>>,
}

impl Default for MockCostSink {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCostSink {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    /// Take all recorded costs, clearing the internal buffer.
    pub async fn take_records(&self) -> Vec<SessionCost> {
        std::mem::take(&mut *self.records.lock().await)
    }

    /// Peek at recorded costs without clearing.
    pub async fn records(&self) -> Vec<SessionCost> {
        self.records.lock().await.clone()
    }
}

impl CostSink for MockCostSink {
    async fn save(&self, costs: &[SessionCost]) -> anyhow::Result<()> {
        let mut guard = self.records.lock().await;
        guard.clear();
        guard.extend_from_slice(costs);
        Ok(())
    }

    async fn load(&self) -> anyhow::Result<Vec<SessionCost>> {
        Ok(self.records.lock().await.clone())
    }
}
