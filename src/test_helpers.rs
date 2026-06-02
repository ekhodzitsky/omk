//! Test helpers for isolated XDG/HOME directory setup and reusable mocks.
//!
//! Intended for use in unit and integration tests to avoid polluting the user's
//! real home directory with test artifacts.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use tokio::sync::Mutex;

use crate::cost::{CostSink, SessionCost};
use crate::runtime::events::{Event, EventSink};

// ---------------------------------------------------------------------------
// Isolated XDG environment
// ---------------------------------------------------------------------------

/// Global mutex for tests that mutate `XDG_STATE_HOME` / `HOME` env vars.
/// Acquire this before calling [`isolated_xdg_env`] and releasing after
/// cleanup to prevent cross-test races.
pub static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
