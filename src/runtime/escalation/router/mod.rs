use std::sync::Arc;

use crate::runtime::classifier::{ClassifierInput, Intent};
use crate::runtime::conversation::{
    bus::{BusEvent, EventBus, PreflightAction},
    outcome::RouteOutcome,
    session::SessionCtx,
};
use crate::runtime::escalation::{
    backends::{ClassifierBackend, GoalBridgeBackend, LlmDirectBackend, WireWorkerBackend},
    preflight::PreflightInbox,
};

mod dispatch;
mod preflight;

#[derive(Debug)]
pub struct RouterConfig {
    pub medium_goal_cap: u32,
    pub cost_cap_usd_soft: Option<f32>,
    pub cost_cap_usd_hard: Option<f32>,
    pub first_prompt_threshold: f32,
    pub normal_disclaimer_threshold: f32,
    pub auto_execute_threshold: f32,
    pub wire_pool_size: u32,
    pub protected_paths: Vec<std::path::PathBuf>,
    pub small_files_preflight_threshold: u32,
    pub preflight_timeout_ms: u32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        use crate::runtime::classifier::{
            CONFIDENCE_AUTO_EXECUTE, CONFIDENCE_FIRST_PROMPT, CONFIDENCE_INLINE_DISCLAIMER,
        };
        Self {
            medium_goal_cap: 3,
            cost_cap_usd_soft: None,
            cost_cap_usd_hard: None,
            first_prompt_threshold: CONFIDENCE_FIRST_PROMPT,
            normal_disclaimer_threshold: CONFIDENCE_INLINE_DISCLAIMER,
            auto_execute_threshold: CONFIDENCE_AUTO_EXECUTE,
            wire_pool_size: 3,
            protected_paths: vec![],
            small_files_preflight_threshold: 5,
            preflight_timeout_ms: 60_000,
        }
    }
}

pub struct Router {
    classifier: Arc<dyn ClassifierBackend>,
    llm_direct: Arc<dyn LlmDirectBackend>,
    wire_worker: Arc<dyn WireWorkerBackend>,
    goal_bridge: Arc<dyn GoalBridgeBackend>,
    pub(crate) config: RouterConfig,
    event_bus: Arc<EventBus>,
    preflight_inbox: PreflightInbox,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Router {
    pub fn new(
        classifier: Arc<dyn ClassifierBackend>,
        llm_direct: Arc<dyn LlmDirectBackend>,
        wire_worker: Arc<dyn WireWorkerBackend>,
        goal_bridge: Arc<dyn GoalBridgeBackend>,
        config: RouterConfig,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            classifier,
            llm_direct,
            wire_worker,
            goal_bridge,
            config,
            event_bus,
            preflight_inbox: PreflightInbox::new(),
        }
    }

    pub async fn submit_preflight(&self, ticket_id: String, action: PreflightAction) {
        self.preflight_inbox.submit(ticket_id, action).await;
    }

    pub async fn dispatch(&self, prompt: &str, session: &Arc<SessionCtx>) -> RouteOutcome {
        self.dispatch_inner(prompt, None, session).await
    }

    pub async fn dispatch_with_intent_override(
        &self,
        prompt: &str,
        override_intent: Intent,
        session: &Arc<SessionCtx>,
    ) -> RouteOutcome {
        self.dispatch_inner(prompt, Some(override_intent), session)
            .await
    }

    async fn dispatch_inner(
        &self,
        prompt: &str,
        override_intent: Option<Intent>,
        session: &Arc<SessionCtx>,
    ) -> RouteOutcome {
        self.maybe_emit_soft_cap_warning(session).await;

        let input = ClassifierInput {
            prompt: prompt.to_string(),
            recent_conversation: vec![],
            project_root: session.project_root.clone(),
        };
        let mut output = self.classifier.classify(input).await;
        if let Some(intent) = override_intent {
            output.intent = intent;
        }

        self.event_bus.publish(BusEvent::ClassifierDecided {
            intent: output.intent,
            confidence: output.confidence,
            latency_ms: output.latency_ms,
            reasoning: output.reasoning.clone(),
            cached: output.source == crate::runtime::classifier::ClassificationSource::Cache,
            fallback: output.fallback,
        });

        let outcome = self.dispatch_from_output(prompt, &output, session).await;

        if !matches!(
            outcome,
            RouteOutcome::Refused { .. } | RouteOutcome::Cancelled
        ) {
            session.first_prompt_done();
        }

        outcome
    }

    async fn maybe_emit_soft_cap_warning(&self, session: &Arc<SessionCtx>) {
        if let Some(soft) = self.config.cost_cap_usd_soft {
            let cost = *session.cumulative_cost_usd.lock().await;
            if cost >= soft
                && session
                    .cost_soft_warned
                    .compare_exchange(
                        false,
                        true,
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Acquire,
                    )
                    .is_ok()
            {
                self.event_bus.publish(BusEvent::CostSoftCapWarning {
                    current_usd: cost,
                    soft_cap_usd: soft,
                });
            }
        }
    }
}
