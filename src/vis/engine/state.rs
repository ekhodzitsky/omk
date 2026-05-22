use std::time::{Duration, Instant};

use crate::vis::bus::{EngineEvent, Intent};

/// Visible size of the engine pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneState {
    #[default]
    Collapsed,
    Compact,
    Expanded,
}

/// Drives pane-state transitions based on events, user input, and idle timeout.
#[derive(Debug)]
pub struct PaneStateMachine {
    pub state: PaneState,
    pub last_event_at: Instant,
    idle_timeout: Duration,
}

impl PaneStateMachine {
    pub fn new() -> Self {
        Self {
            state: PaneState::Collapsed,
            last_event_at: Instant::now(),
            idle_timeout: Duration::from_secs(60),
        }
    }

    pub fn state(&self) -> PaneState {
        self.state
    }

    /// React to an incoming engine event.
    pub fn on_event(&mut self, ev: &EngineEvent, now: Instant) {
        let should_expand = matches!(
            ev,
            EngineEvent::RouterEscalating {
                intent: Intent::Small | Intent::Medium | Intent::Large,
                ..
            } | EngineEvent::WorkerStarted { .. }
                | EngineEvent::GoalCreated { .. }
        );

        if should_expand && self.state == PaneState::Collapsed {
            self.state = PaneState::Compact;
        }

        self.last_event_at = now;
    }

    /// Periodic tick — auto-collapse on idle.
    pub fn on_tick(&mut self, now: Instant) {
        if now.duration_since(self.last_event_at) > self.idle_timeout
            && (self.state == PaneState::Expanded || self.state == PaneState::Compact)
        {
            self.state = PaneState::Collapsed;
        }
    }

    /// User pressed Tab — toggle Compact <-> Expanded.
    pub fn user_tab(&mut self) {
        self.state = match self.state {
            PaneState::Collapsed => PaneState::Compact,
            PaneState::Compact => PaneState::Expanded,
            PaneState::Expanded => PaneState::Compact,
        };
    }

    /// User pressed Shift-Tab — collapse.
    pub fn user_shift_tab(&mut self) {
        self.state = PaneState::Collapsed;
    }
}

impl Default for PaneStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vis::bus::ActiveMode;

    #[test]
    fn auto_expand_on_router_escalating_non_trivial() {
        let mut sm = PaneStateMachine::new();
        sm.state = PaneState::Collapsed;
        let now = Instant::now();
        sm.on_event(
            &EngineEvent::RouterEscalating {
                intent: Intent::Small,
                target_mode: ActiveMode::WireWorker,
                preflight: false,
            },
            now,
        );
        assert_eq!(sm.state(), PaneState::Compact);
    }

    #[test]
    fn no_auto_expand_on_trivial_router_escalating() {
        let mut sm = PaneStateMachine::new();
        sm.state = PaneState::Collapsed;
        let now = Instant::now();
        sm.on_event(
            &EngineEvent::RouterEscalating {
                intent: Intent::Trivial,
                target_mode: ActiveMode::DirectLlm,
                preflight: false,
            },
            now,
        );
        assert_eq!(sm.state(), PaneState::Collapsed);
    }

    #[test]
    fn auto_expand_on_worker_started() {
        let mut sm = PaneStateMachine::new();
        sm.state = PaneState::Collapsed;
        let now = Instant::now();
        sm.on_event(
            &EngineEvent::WorkerStarted {
                worker_id: "w1".into(),
                kind: "edit".into(),
                task: "rename".into(),
            },
            now,
        );
        assert_eq!(sm.state(), PaneState::Compact);
    }

    #[test]
    fn auto_collapse_after_idle() {
        let mut sm = PaneStateMachine::new();
        sm.state = PaneState::Expanded;
        let now = Instant::now();
        sm.last_event_at = now - Duration::from_secs(70);
        sm.on_tick(now);
        assert_eq!(sm.state(), PaneState::Collapsed);
    }

    #[test]
    fn user_tab_toggles() {
        let mut sm = PaneStateMachine::new();
        sm.state = PaneState::Collapsed;
        sm.user_tab();
        assert_eq!(sm.state(), PaneState::Compact);
        sm.user_tab();
        assert_eq!(sm.state(), PaneState::Expanded);
        sm.user_tab();
        assert_eq!(sm.state(), PaneState::Compact);
    }

    #[test]
    fn user_shift_tab_collapses() {
        let mut sm = PaneStateMachine::new();
        sm.state = PaneState::Expanded;
        sm.user_shift_tab();
        assert_eq!(sm.state(), PaneState::Collapsed);
    }
}
