use std::sync::Arc;

use crate::runtime::classifier::Intent;
use crate::runtime::conversation::{outcome::RouteOutcome, session::SessionCtx};
use crate::runtime::escalation::router::Router;

pub async fn handle_quick(
    router: &Router,
    prompt: &str,
    session: &Arc<SessionCtx>,
) -> RouteOutcome {
    router
        .dispatch_with_intent_override(prompt, Intent::Small, session)
        .await
}

pub async fn handle_escalate(
    router: &Router,
    prompt: &str,
    session: &Arc<SessionCtx>,
) -> RouteOutcome {
    router
        .dispatch_with_intent_override(prompt, Intent::Large, session)
        .await
}
