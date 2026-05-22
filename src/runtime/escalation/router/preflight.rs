use std::sync::Arc;
use std::time::Duration;

use crate::runtime::classifier::{ClassifierOutput, Intent};
use crate::runtime::conversation::{
    bus::{BusEvent, Preflight, PreflightAction, PreflightKind},
    outcome::RouteOutcome,
    session::SessionCtx,
};

use super::Router;

impl Router {
    pub(super) async fn preflight_kind(
        &self,
        output: &ClassifierOutput,
        prompt: &str,
        session: &Arc<SessionCtx>,
    ) -> Option<PreflightKind> {
        match output.intent {
            Intent::Trivial => None,
            Intent::Large => {
                if session.active_large_goal.lock().await.is_some() {
                    return Some(PreflightKind::QueueLargeOnActiveLarge);
                }
                Some(PreflightKind::LargeEscalation)
            }
            Intent::Medium => {
                if session.is_first() && output.confidence < self.config.first_prompt_threshold {
                    return Some(PreflightKind::MediumLowConfidence);
                }
                if output.confidence < self.config.normal_disclaimer_threshold {
                    return Some(PreflightKind::MediumLowConfidence);
                }
                let active = session.active_medium_goals.lock().await.len();
                if active >= self.config.medium_goal_cap as usize {
                    return Some(PreflightKind::QueueMediumAtConcurrencyCap);
                }
                None
            }
            Intent::Small => {
                if session.is_first() && output.confidence < self.config.first_prompt_threshold {
                    return Some(PreflightKind::MediumLowConfidence);
                }
                if output.confidence < self.config.normal_disclaimer_threshold {
                    return Some(PreflightKind::MediumLowConfidence);
                }
                if self.involves_protected_path(prompt) {
                    return Some(PreflightKind::SmallOverProtectedPath);
                }
                let file_count = self.estimate_file_count(prompt);
                if file_count >= self.config.small_files_preflight_threshold {
                    return Some(PreflightKind::SmallOverManyFiles);
                }
                None
            }
        }
    }

    fn involves_protected_path(&self, prompt: &str) -> bool {
        for path in &self.config.protected_paths {
            if let Some(s) = path.to_str() {
                if prompt.contains(s) {
                    return true;
                }
            }
        }
        false
    }

    fn estimate_file_count(&self, prompt: &str) -> u32 {
        prompt
            .split_whitespace()
            .filter(|w| w.contains('.') || w.contains('/'))
            .count() as u32
    }

    pub(super) async fn run_preflight(
        &self,
        kind: PreflightKind,
        output: &ClassifierOutput,
    ) -> PreflightAction {
        let headline = match kind {
            PreflightKind::LargeEscalation => "Large escalation: launch goal-mode?".to_string(),
            PreflightKind::MediumLowConfidence => format!(
                "Low confidence ({:.2}): proceed with {}?",
                output.confidence,
                format!("{:?}", output.intent).to_lowercase()
            ),
            PreflightKind::SmallOverProtectedPath => {
                "Protected path touched: proceed with small edit?".to_string()
            }
            PreflightKind::SmallOverManyFiles => {
                "Many files affected: proceed with small edit?".to_string()
            }
            PreflightKind::QueueLargeOnActiveLarge => {
                "Large goal already active: queue this one?".to_string()
            }
            PreflightKind::QueueMediumAtConcurrencyCap => format!(
                "At concurrency cap ({}): this will queue",
                self.config.medium_goal_cap
            ),
        };
        let ticket = self.preflight_inbox.arm().await;
        let preflight = Preflight {
            kind,
            headline,
            timeout_ms: self.config.preflight_timeout_ms,
            ticket_id: ticket.id.clone(),
        };
        self.event_bus
            .publish(BusEvent::PreflightRequest(preflight.clone()));
        let timeout = tokio::time::sleep(Duration::from_millis(preflight.timeout_ms as u64));
        tokio::select! {
            action = ticket.rx => {
                let action = action.unwrap_or(PreflightAction::Timeout);
                self.event_bus.publish(BusEvent::PreflightResponse(action));
                action
            }
            _ = timeout => {
                self.preflight_inbox.cancel(&ticket.id).await;
                self.event_bus.publish(BusEvent::PreflightResponse(PreflightAction::Timeout));
                PreflightAction::Timeout
            }
        }
    }

    pub(super) async fn handle_preflight_action(
        &self,
        prompt: &str,
        output: &ClassifierOutput,
        action: PreflightAction,
        session: &Arc<SessionCtx>,
    ) -> RouteOutcome {
        match action {
            PreflightAction::Accept => {
                // If the preflight was due to a concurrency cap, actually queue.
                let queued = match output.intent {
                    Intent::Medium => {
                        let active = session.active_medium_goals.lock().await.len();
                        active >= self.config.medium_goal_cap as usize
                    }
                    Intent::Large => session.active_large_goal.lock().await.is_some(),
                    _ => false,
                };
                if queued {
                    return RouteOutcome::Queued {
                        intent: output.intent,
                        position: session.active_medium_goals.lock().await.len(),
                    };
                }
                self.dispatch_direct(prompt, output, session).await
            }
            PreflightAction::Explain => {
                let line = format!("Explain: {}", output.reasoning);
                self.event_bus.publish(BusEvent::DisclosureLine(line));
                RouteOutcome::Cancelled
            }
            PreflightAction::Downgrade => {
                let (new_intent, log_msg) = match output.intent {
                    Intent::Large => (Intent::Medium, None),
                    Intent::Medium => (Intent::Small, None),
                    Intent::Small => {
                        let msg =
                            "already lowest non-trivial; type more specific prompt or use /quick"
                                .to_string();
                        self.event_bus
                            .publish(BusEvent::DisclosureLine(msg.clone()));
                        (Intent::Small, Some(msg))
                    }
                    Intent::Trivial => (Intent::Trivial, None),
                };
                if let Some(msg) = log_msg {
                    return RouteOutcome::Refused { reason: msg };
                }
                let mut downgraded_output = output.clone();
                downgraded_output.intent = new_intent;
                let inner = self
                    .dispatch_direct(prompt, &downgraded_output, session)
                    .await;
                RouteOutcome::Downgraded {
                    from: output.intent,
                    to: new_intent,
                    outcome: Box::new(inner),
                }
            }
            PreflightAction::Cancel | PreflightAction::Timeout => RouteOutcome::Cancelled,
        }
    }
}
