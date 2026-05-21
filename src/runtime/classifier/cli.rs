use std::path::Path;

use anyhow::Result;
use lru::LruCache;

use super::{classify, telemetry, types::ClassifierInput, ClassifierOutput, LlmClassifierBackend};

pub async fn handle_classify_command(
    args: &str,
    backend: &dyn LlmClassifierBackend,
    cache: &mut LruCache<u64, ClassifierOutput>,
    project_root: &Path,
    session_id: &str,
) -> Result<ClassifierOutput> {
    let prompt = args.trim().to_string();
    let input = ClassifierInput {
        prompt,
        recent_conversation: vec![],
        project_root: project_root.to_owned(),
    };
    let output = classify(input, backend, cache).await;

    let event = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "kind": "classifier.decided",
        "intent": output.intent,
        "confidence": output.confidence,
        "latency_ms": output.latency_ms,
        "reasoning": output.reasoning,
        "source": output.source,
    });
    let session_dir = crate::runtime::config::state_dir()
        .join("sessions")
        .join(session_id);
    telemetry::write_engine_event(&session_dir, &event).await?;

    Ok(output)
}
