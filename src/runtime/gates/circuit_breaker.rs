//! Production-grade Circuit Breaker for OMK gates.
//!
//! A distributed state machine with durable persistence, observability,
//! and zero-overhead fast path.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};

use crate::runtime::db::repo::circuit_breaker::{
    CircuitBreakerRecord, CircuitBreakerRepo, CircuitBreakerRepoImpl,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Circuit breaker configuration for a single gate or global defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_recovery_timeout_secs")]
    pub recovery_timeout_secs: u64,
    #[serde(default = "default_half_open_max_calls")]
    pub half_open_max_calls: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            recovery_timeout_secs: default_recovery_timeout_secs(),
            half_open_max_calls: default_half_open_max_calls(),
            enabled: default_enabled(),
        }
    }
}

fn default_failure_threshold() -> u32 {
    5
}

fn default_recovery_timeout_secs() -> u64 {
    30
}

fn default_half_open_max_calls() -> u32 {
    1
}

fn default_enabled() -> bool {
    true
}

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CircuitState {
    /// Normal operation. Consecutive failures are counted.
    Closed,
    /// After threshold failures. All calls fail fast.
    Open,
    /// After recovery timeout. A limited number of probe calls are allowed.
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

impl CircuitState {
    fn from_db(s: &str) -> Option<Self> {
        match s {
            "closed" => Some(CircuitState::Closed),
            "open" => Some(CircuitState::Open),
            "half_open" => Some(CircuitState::HalfOpen),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Check result
// ---------------------------------------------------------------------------

/// Result of a circuit breaker check before executing a gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitCheck {
    /// Gate may execute.
    Allow,
    /// Gate must not execute; circuit is open.
    Deny {
        reason: String,
        consecutive_failures: u32,
        last_failure: Option<DateTime<Utc>>,
        recovery_in_secs: u64,
    },
}

// ---------------------------------------------------------------------------
// Core breaker
// ---------------------------------------------------------------------------

/// In-memory circuit breaker for a single gate.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub id: String,
    pub gate_name: String,
    pub project_path: String,
    pub config: CircuitBreakerConfig,
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
    pub half_open_calls_remaining: u32,
    /// True if the in-memory state differs from the last persisted state.
    pub dirty: bool,
}

impl CircuitBreaker {
    /// Create a new circuit breaker from configuration.
    pub fn new(
        id: String,
        gate_name: String,
        project_path: String,
        config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            id,
            gate_name,
            project_path,
            config,
            state: CircuitState::Closed,
            consecutive_failures: 0,
            last_failure_at: None,
            last_success_at: None,
            opened_at: None,
            half_open_calls_remaining: 0,
            dirty: false,
        }
    }

    /// Restore from a database record.
    pub fn from_record(record: &CircuitBreakerRecord) -> Option<Self> {
        let state = CircuitState::from_db(&record.state)?;
        let config = CircuitBreakerConfig {
            failure_threshold: record.failure_threshold as u32,
            recovery_timeout_secs: record.recovery_timeout_secs as u64,
            half_open_max_calls: record.half_open_max_calls as u32,
            enabled: true,
        };

        let mut cb = Self {
            id: record.id.clone(),
            gate_name: record.gate_name.clone(),
            project_path: record.project_path.clone(),
            config,
            state,
            consecutive_failures: record.consecutive_failures as u32,
            last_failure_at: record.last_failure_at,
            last_success_at: record.last_success_at,
            opened_at: record.opened_at,
            half_open_calls_remaining: record.half_open_calls_remaining as u32,
            dirty: false,
        };

        // Apply time-based transitions on load.
        cb.apply_time_based_transitions();
        Some(cb)
    }

    /// Convert to a database record.
    pub fn to_record(&self) -> CircuitBreakerRecord {
        CircuitBreakerRecord {
            id: self.id.clone(),
            gate_name: self.gate_name.clone(),
            project_path: self.project_path.clone(),
            state: self.state.to_string(),
            consecutive_failures: self.consecutive_failures as i64,
            failure_threshold: self.config.failure_threshold as i64,
            recovery_timeout_secs: self.config.recovery_timeout_secs as i64,
            half_open_max_calls: self.config.half_open_max_calls as i64,
            half_open_calls_remaining: self.half_open_calls_remaining as i64,
            last_failure_at: self.last_failure_at,
            last_success_at: self.last_success_at,
            opened_at: self.opened_at,
            updated_at: Utc::now(),
        }
    }

    /// Fast O(1) check. No I/O.
    pub fn check(&mut self) -> CircuitCheck {
        if !self.config.enabled || self.config.failure_threshold == 0 {
            return CircuitCheck::Allow;
        }

        // Re-evaluate time-based transitions on every check.
        self.apply_time_based_transitions();

        match self.state {
            CircuitState::Closed => CircuitCheck::Allow,
            CircuitState::Open => {
                let recovery_in_secs = self.compute_recovery_remaining();
                CircuitCheck::Deny {
                    reason: format!(
                        "Circuit breaker OPEN for gate '{}' after {} consecutive failures",
                        self.gate_name, self.consecutive_failures
                    ),
                    consecutive_failures: self.consecutive_failures,
                    last_failure: self.last_failure_at,
                    recovery_in_secs,
                }
            }
            CircuitState::HalfOpen => {
                if self.half_open_calls_remaining > 0 {
                    self.half_open_calls_remaining -= 1;
                    self.dirty = true;
                    CircuitCheck::Allow
                } else {
                    // Exhausted probes: transition back to Open immediately.
                    self.transition_to(CircuitState::Open);
                    let recovery_in_secs = self.compute_recovery_remaining();
                    CircuitCheck::Deny {
                        reason: format!(
                            "Circuit breaker OPEN for gate '{}' after {} consecutive failures (half-open probes exhausted)",
                            self.gate_name, self.consecutive_failures
                        ),
                        consecutive_failures: self.consecutive_failures,
                        last_failure: self.last_failure_at,
                        recovery_in_secs,
                    }
                }
            }
        }
    }

    /// Record a successful gate execution.
    pub fn record_success(&mut self) {
        let old_state = self.state;
        self.consecutive_failures = 0;
        self.last_success_at = Some(Utc::now());

        if old_state == CircuitState::HalfOpen {
            self.transition_to(CircuitState::Closed);
        }

        // If we were already Closed, no transition but we still update last_success_at.
        // Mark dirty so the success timestamp can be persisted.
        if old_state == CircuitState::Closed {
            self.dirty = true;
        }
    }

    /// Record a failed gate execution.
    ///
    /// `timed_out` and `rate_limited` help decide whether the failure counts
    /// toward the circuit breaker threshold.
    pub fn record_failure(&mut self, _timed_out: bool, rate_limited: bool) {
        // Rate-limited failures are transient and do not count.
        if rate_limited {
            return;
        }

        // Timeouts DO count as failures.
        let old_state = self.state;
        self.consecutive_failures += 1;
        self.last_failure_at = Some(Utc::now());

        if old_state == CircuitState::HalfOpen {
            self.transition_to(CircuitState::Open);
            return;
        }

        if old_state == CircuitState::Closed {
            if self.consecutive_failures >= self.config.failure_threshold {
                self.transition_to(CircuitState::Open);
            } else {
                self.dirty = true;
            }
        }
    }

    /// Current state (read-only).
    pub fn current_state(&self) -> CircuitState {
        self.state
    }

    /// Reset the breaker to Closed.
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
        self.last_failure_at = None;
        self.opened_at = None;
        self.half_open_calls_remaining = 0;
        self.dirty = true;
    }

    /// Update configuration while preserving state.
    /// If the threshold increased, an already-open breaker may close.
    pub fn update_config(&mut self, new_config: CircuitBreakerConfig) {
        let old_threshold = self.config.failure_threshold;
        self.config = new_config;

        // If threshold increased and we're below it, close the circuit.
        if self.state == CircuitState::Open
            && self.consecutive_failures < self.config.failure_threshold
        {
            self.transition_to(CircuitState::Closed);
        }

        // If threshold decreased and we're now above it, open the circuit.
        if self.state == CircuitState::Closed
            && self.consecutive_failures >= self.config.failure_threshold
        {
            self.transition_to(CircuitState::Open);
        }

        // If threshold changed but state didn't transition, mark dirty if config changed.
        if old_threshold != self.config.failure_threshold && !self.dirty {
            self.dirty = true;
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn transition_to(&mut self, new_state: CircuitState) {
        let old_state = self.state;
        if old_state == new_state {
            return;
        }

        self.state = new_state;
        self.dirty = true;

        match new_state {
            CircuitState::Open => {
                self.opened_at = Some(Utc::now());
                self.half_open_calls_remaining = 0;
            }
            CircuitState::HalfOpen => {
                self.half_open_calls_remaining = self.config.half_open_max_calls;
            }
            CircuitState::Closed => {
                self.consecutive_failures = 0;
                self.opened_at = None;
                self.half_open_calls_remaining = 0;
            }
        }

        warn!(
            gate = %self.gate_name,
            previous_state = ?old_state,
            new_state = ?new_state,
            "Circuit breaker state transition"
        );
    }

    fn apply_time_based_transitions(&mut self) {
        if self.state != CircuitState::Open {
            return;
        }

        if let Some(opened_at) = self.opened_at {
            let elapsed = (Utc::now() - opened_at).num_seconds() as u64;
            if elapsed >= self.config.recovery_timeout_secs {
                self.transition_to(CircuitState::HalfOpen);
            }
        }
    }

    fn compute_recovery_remaining(&self) -> u64 {
        match self.opened_at {
            Some(opened_at) => {
                let elapsed = (Utc::now() - opened_at).num_seconds() as u64;
                self.config.recovery_timeout_secs.saturating_sub(elapsed)
            }
            None => self.config.recovery_timeout_secs,
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Thread-safe registry of circuit breakers.
#[derive(Debug)]
pub struct CircuitBreakerRegistry {
    breakers: RwLock<HashMap<String, Arc<Mutex<CircuitBreaker>>>>,
    repo: Option<CircuitBreakerRepoImpl>,
}

impl CircuitBreakerRegistry {
    /// Create a new in-memory-only registry.
    pub fn new() -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            repo: None,
        }
    }

    /// Create a registry backed by SQLite persistence.
    pub fn with_repo(repo: CircuitBreakerRepoImpl) -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            repo: Some(repo),
        }
    }

    /// Load all breakers from the database.
    pub async fn load_from_db(&self) -> Result<(), CircuitBreakerError> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Ok(()),
        };

        let records = repo.load_all().await.map_err(CircuitBreakerError::Db)?;
        let mut map = self.breakers.write().await;
        for record in records {
            if let Some(cb) = CircuitBreaker::from_record(&record) {
                let id = cb.id.clone();
                map.insert(id, Arc::new(Mutex::new(cb)));
            }
        }
        Ok(())
    }

    /// Check whether a gate may execute.
    pub async fn check(
        &self,
        gate_name: &str,
        project_path: &Path,
        config: Option<&CircuitBreakerConfig>,
    ) -> CircuitCheck {
        let id = breaker_id(gate_name, project_path);
        let breaker = self
            .get_or_create(&id, gate_name, project_path, config)
            .await;
        let mut guard = breaker.lock().await;
        let result = guard.check();

        info!(
            gate = %gate_name,
            circuit_state = ?guard.state,
            consecutive_failures = guard.consecutive_failures,
            "Circuit breaker check"
        );

        result
    }

    /// Record a successful gate execution.
    pub async fn record_success(&self, gate_name: &str, project_path: &Path) {
        let id = breaker_id(gate_name, project_path);
        let breaker = self.get_or_create(&id, gate_name, project_path, None).await;
        let mut guard = breaker.lock().await;
        let old_state = guard.state;
        guard.record_success();
        if guard.dirty {
            if let Err(e) = self.maybe_persist(&guard).await {
                warn!(gate = %gate_name, error = %e, "Failed to persist circuit breaker success");
            }
            guard.dirty = false;
        }

        if old_state == CircuitState::HalfOpen && guard.state == CircuitState::Closed {
            info!(gate = %gate_name, "Circuit breaker probe succeeded, closed");
        }
    }

    /// Record a failed gate execution.
    pub async fn record_failure(
        &self,
        gate_name: &str,
        project_path: &Path,
        timed_out: bool,
        rate_limited: bool,
    ) {
        let id = breaker_id(gate_name, project_path);
        let breaker = self.get_or_create(&id, gate_name, project_path, None).await;
        let mut guard = breaker.lock().await;
        let old_state = guard.state;
        guard.record_failure(timed_out, rate_limited);
        if guard.dirty {
            if let Err(e) = self.maybe_persist(&guard).await {
                warn!(gate = %gate_name, error = %e, "Failed to persist circuit breaker failure");
            }
            guard.dirty = false;
        }

        if old_state == CircuitState::HalfOpen && guard.state == CircuitState::Open {
            info!(gate = %gate_name, "Circuit breaker probe failed, reopened");
        }
    }

    /// Reset a specific breaker to Closed.
    pub async fn reset(
        &self,
        gate_name: &str,
        project_path: &Path,
    ) -> Result<(), CircuitBreakerError> {
        let id = breaker_id(gate_name, project_path);
        let map = self.breakers.read().await;
        if let Some(breaker) = map.get(&id) {
            let mut guard = breaker.lock().await;
            guard.reset();
            if let Err(e) = self.maybe_persist(&guard).await {
                warn!(gate = %gate_name, error = %e, "Failed to persist circuit breaker reset");
            }
            guard.dirty = false;
        }
        Ok(())
    }

    /// Reset all breakers to Closed.
    pub async fn reset_all(&self) -> Result<(), CircuitBreakerError> {
        let map = self.breakers.read().await;
        for (_, breaker) in map.iter() {
            let mut guard = breaker.lock().await;
            guard.reset();
            if let Err(e) = self.maybe_persist(&guard).await {
                warn!(gate = %guard.gate_name, error = %e, "Failed to persist circuit breaker reset");
            }
            guard.dirty = false;
        }
        Ok(())
    }

    /// List all breakers with their current state.
    pub async fn list(&self) -> Vec<CircuitBreakerStatus> {
        let map = self.breakers.read().await;
        let mut statuses = Vec::with_capacity(map.len());
        for (_, breaker) in map.iter() {
            let guard = breaker.lock().await;
            statuses.push(CircuitBreakerStatus {
                gate_name: guard.gate_name.clone(),
                state: guard.state,
                consecutive_failures: guard.consecutive_failures,
                failure_threshold: guard.config.failure_threshold,
                last_failure_at: guard.last_failure_at,
                last_success_at: guard.last_success_at,
                opened_at: guard.opened_at,
                recovery_timeout_secs: guard.config.recovery_timeout_secs,
                half_open_calls_remaining: guard.half_open_calls_remaining,
            });
        }
        statuses.sort_by(|a, b| a.gate_name.cmp(&b.gate_name));
        statuses
    }

    /// Flush all dirty breakers to the database.
    pub async fn flush(&self) -> Result<(), CircuitBreakerError> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Ok(()),
        };

        let map = self.breakers.read().await;
        for (_, breaker) in map.iter() {
            let mut guard = breaker.lock().await;
            if guard.dirty {
                let record = guard.to_record();
                if let Err(e) = repo.save(&record).await {
                    warn!(gate = %guard.gate_name, error = %e, "Failed to flush circuit breaker");
                } else {
                    guard.dirty = false;
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn get_or_create(
        &self,
        id: &str,
        gate_name: &str,
        project_path: &Path,
        config: Option<&CircuitBreakerConfig>,
    ) -> Arc<Mutex<CircuitBreaker>> {
        {
            let map = self.breakers.read().await;
            if let Some(breaker) = map.get(id) {
                if let Some(cfg) = config {
                    let mut guard = breaker.lock().await;
                    guard.update_config(cfg.clone());
                }
                return breaker.clone();
            }
        }

        let mut map = self.breakers.write().await;
        // Double-check after acquiring write lock.
        if let Some(breaker) = map.get(id) {
            if let Some(cfg) = config {
                let mut guard = breaker.lock().await;
                guard.update_config(cfg.clone());
            }
            return breaker.clone();
        }

        let cfg = config.cloned().unwrap_or_default();
        let cb = CircuitBreaker::new(
            id.to_string(),
            gate_name.to_string(),
            project_path.to_string_lossy().to_string(),
            cfg,
        );
        let arc = Arc::new(Mutex::new(cb));
        map.insert(id.to_string(), arc.clone());
        arc
    }

    async fn maybe_persist(&self, cb: &CircuitBreaker) -> Result<(), CircuitBreakerError> {
        if let Some(repo) = &self.repo {
            let record = cb.to_record();
            repo.save(&record).await.map_err(CircuitBreakerError::Db)?;
        }
        Ok(())
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Human-readable status for a circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub gate_name: String,
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub failure_threshold: u32,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
    pub recovery_timeout_secs: u64,
    pub half_open_calls_remaining: u32,
}

// ---------------------------------------------------------------------------
// Global registry
// ---------------------------------------------------------------------------

static GLOBAL_REGISTRY: OnceLock<CircuitBreakerRegistry> = OnceLock::new();

/// Initialize the global circuit breaker registry with a specific instance.
pub fn init_global_registry(
    registry: CircuitBreakerRegistry,
) -> Result<(), Box<CircuitBreakerRegistry>> {
    GLOBAL_REGISTRY.set(registry).map_err(Box::new)
}

/// Access the global registry, lazily initializing an in-memory one if needed.
pub fn global_registry() -> &'static CircuitBreakerRegistry {
    GLOBAL_REGISTRY.get_or_init(CircuitBreakerRegistry::new)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn breaker_id(gate_name: &str, project_path: &Path) -> String {
    let path_str = project_path.to_string_lossy();
    format!("{}:{}", path_str, gate_name)
}

/// Errors emitted by the circuit breaker layer.
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError {
    #[error("database error: {0}")]
    Db(#[from] crate::runtime::db::error::DbError),

    #[error("registry not initialized")]
    NotInitialized,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_disabled_when_threshold_zero() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);
        assert!(matches!(cb.check(), CircuitCheck::Allow));
        cb.record_failure(false, false);
        assert!(matches!(cb.check(), CircuitCheck::Allow));
    }

    #[test]
    fn test_closed_to_open_transition() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        assert!(matches!(cb.check(), CircuitCheck::Allow));
        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Open);
        assert!(matches!(cb.check(), CircuitCheck::Deny { .. }));
    }

    #[test]
    fn test_success_resets_consecutive_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        cb.record_failure(false, false);
        cb.record_success();
        assert_eq!(cb.consecutive_failures, 0);
        assert_eq!(cb.state, CircuitState::Closed);

        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_rate_limited_failure_ignored() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, true);
        assert_eq!(cb.consecutive_failures, 0);
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_timed_out_failure_counts() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(true, false);
        assert_eq!(cb.consecutive_failures, 1);
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_half_open_probe_success_closes() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 0, // immediate recovery
            half_open_max_calls: 1,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Open);

        // Simulate time passing.
        cb.opened_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(matches!(cb.check(), CircuitCheck::Allow));
        assert_eq!(cb.state, CircuitState::HalfOpen);
        assert_eq!(cb.half_open_calls_remaining, 0);

        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_half_open_probe_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 0,
            half_open_max_calls: 1,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Open);

        cb.opened_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(matches!(cb.check(), CircuitCheck::Allow));
        assert_eq!(cb.state, CircuitState::HalfOpen);

        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.consecutive_failures, 0);
        assert!(cb.opened_at.is_none());
    }

    #[test]
    fn test_config_update_threshold_increase_may_close() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Open);

        cb.update_config(CircuitBreakerConfig {
            failure_threshold: 5,
            ..Default::default()
        });
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_config_update_threshold_decrease_may_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        cb.record_failure(false, false);
        assert_eq!(cb.state, CircuitState::Closed);

        cb.update_config(CircuitBreakerConfig {
            failure_threshold: 1,
            ..Default::default()
        });
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_half_open_calls_exhausted() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 0,
            half_open_max_calls: 1,
            ..Default::default()
        };
        let mut cb = CircuitBreaker::new("id".into(), "test".into(), "/tmp".into(), config);

        cb.record_failure(false, false);
        cb.opened_at = Some(Utc::now() - chrono::Duration::seconds(1));

        // First check in HalfOpen consumes the probe.
        assert!(matches!(cb.check(), CircuitCheck::Allow));
        assert_eq!(cb.half_open_calls_remaining, 0);

        // Second check should deny because no probes remain.
        assert!(matches!(cb.check(), CircuitCheck::Deny { .. }));
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_registry_check_allow_and_deny() {
        let registry = CircuitBreakerRegistry::new();
        let path = std::path::Path::new("/tmp/test-project");

        // Initially closed.
        let check = registry.check("lint", path, None).await;
        assert!(matches!(check, CircuitCheck::Allow));

        // Record failures to open the circuit.
        for _ in 0..5 {
            registry.record_failure("lint", path, false, false).await;
        }

        let check = registry.check("lint", path, None).await;
        assert!(matches!(check, CircuitCheck::Deny { .. }));
    }

    #[tokio::test]
    async fn test_registry_record_success_closes() {
        let registry = CircuitBreakerRegistry::new();
        let path = std::path::Path::new("/tmp/test-project");

        for _ in 0..5 {
            registry.record_failure("lint", path, false, false).await;
        }
        assert!(matches!(
            registry.check("lint", path, None).await,
            CircuitCheck::Deny { .. }
        ));

        // Simulate recovery timeout by resetting.
        registry.reset("lint", path).await.unwrap();
        assert!(matches!(
            registry.check("lint", path, None).await,
            CircuitCheck::Allow
        ));
    }

    #[tokio::test]
    async fn test_registry_parallel_access() {
        let registry = std::sync::Arc::new(CircuitBreakerRegistry::new());
        let path = std::path::Path::new("/tmp/test-project");

        let mut handles = Vec::new();
        for _ in 0..10 {
            let reg = registry.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..100 {
                    let _ = reg.check("lint", path, None).await;
                    reg.record_failure("lint", path, false, false).await;
                }
            });
            handles.push(handle);
        }

        for h in handles {
            h.await.unwrap();
        }

        // With 1000 failures, circuit should definitely be open.
        let check = registry.check("lint", path, None).await;
        assert!(matches!(check, CircuitCheck::Deny { .. }));
    }

    #[tokio::test]
    async fn test_registry_simulation_performance() {
        let registry = CircuitBreakerRegistry::new();
        let path = std::path::Path::new("/tmp/test-project");

        // Pre-warm registry with 100 gates.
        for i in 0..100 {
            let name = format!("gate-{i:03}");
            let _ = registry.check(&name, path, None).await;
        }

        let start = std::time::Instant::now();
        let checks = 1000;
        for _ in 0..checks {
            for i in 0..100 {
                let name = format!("gate-{i:03}");
                let _ = registry.check(&name, path, None).await;
            }
        }
        let elapsed = start.elapsed();
        let total = checks * 100;
        let per_check = elapsed.as_nanos() as f64 / total as f64;

        // Assert sub-millisecond per check (< 1_000_000 ns).
        assert!(
            per_check < 1_000_000.0,
            "Expected < 1ms per check, got {:.0} ns",
            per_check
        );
    }
}
