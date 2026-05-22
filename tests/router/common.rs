use std::path::PathBuf;
use std::sync::Arc;

use omk::runtime::classifier::{ClassificationSource, ClassifierOutput, Intent};
use omk::runtime::conversation::session::SessionCtx;
use omk::runtime::goal::chat_api::ChildGoalHandle;

pub fn make_session() -> Arc<SessionCtx> {
    SessionCtx::new("test-session".to_string(), PathBuf::from("/tmp"))
}

pub fn make_classifier_output(intent: Intent, confidence: f32) -> ClassifierOutput {
    ClassifierOutput {
        intent,
        confidence,
        reasoning: "test".to_string(),
        signals: vec![],
        suggested_action: None,
        latency_ms: 5,
        source: ClassificationSource::Llm,
        fallback: false,
    }
}

pub fn make_handle(goal_id: &str) -> ChildGoalHandle {
    ChildGoalHandle {
        goal_id: goal_id.to_string(),
        session_id: "test".to_string(),
        created_at: chrono::Utc::now(),
    }
}
