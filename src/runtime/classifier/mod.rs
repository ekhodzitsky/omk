pub mod cache;
pub mod cli;
pub mod heuristic;
pub mod llm_backend;
pub mod system_prompt;
pub mod telemetry;

mod errors;
mod types;

pub use cache::{cache_key, invalidate, new_session_cache};
pub use cli::handle_classify_command;
pub use errors::ClassifierError;
pub use llm_backend::{
    LlmClassifierBackend, MockLlmClassifier, RawLlmClassification, WireLlmClassifierBackend,
};
pub use types::{
    ClassificationSource, ClassifierInput, ClassifierOutput, ConversationTurn, Intent, Role, Signal,
};

pub const CONFIDENCE_AUTO_EXECUTE: f32 = 0.85;
pub const CONFIDENCE_INLINE_DISCLAIMER: f32 = 0.65;
pub const CONFIDENCE_FIRST_PROMPT: f32 = 0.85;

use std::time::Instant;

use heuristic::{heuristic_classify, HeuristicOutcome};
use lru::LruCache;
use telemetry::{append, prompt_hash_hex, TelemetryRecord};

pub async fn classify(
    input: ClassifierInput,
    backend: &dyn LlmClassifierBackend,
    cache: &tokio::sync::Mutex<LruCache<u64, ClassifierOutput>>,
) -> ClassifierOutput {
    // a) telemetry compact if stale
    let _ = telemetry::compact_if_stale(30).await;

    // b) heuristic prefilter
    match heuristic_classify(&input.prompt) {
        HeuristicOutcome::Empty => {
            let output = ClassifierOutput {
                intent: Intent::Small,
                confidence: 0.5,
                reasoning: "input is empty or whitespace".to_string(),
                signals: vec![],
                suggested_action: None,
                latency_ms: 0,
                source: ClassificationSource::Heuristic,
                fallback: false,
            };
            let _ = append(TelemetryRecord {
                ts: chrono::Utc::now(),
                intent: output.intent,
                confidence: output.confidence,
                source: output.source,
                latency_ms: output.latency_ms,
                prompt_hash: prompt_hash_hex(&input.prompt),
                fallback: output.fallback,
            })
            .await;
            return output;
        }
        HeuristicOutcome::SlashCommand => {
            let output = ClassifierOutput {
                intent: Intent::Small,
                confidence: 0.5,
                reasoning: "input is a slash command".to_string(),
                signals: vec![],
                suggested_action: None,
                latency_ms: 0,
                source: ClassificationSource::Heuristic,
                fallback: false,
            };
            let _ = append(TelemetryRecord {
                ts: chrono::Utc::now(),
                intent: output.intent,
                confidence: output.confidence,
                source: output.source,
                latency_ms: output.latency_ms,
                prompt_hash: prompt_hash_hex(&input.prompt),
                fallback: output.fallback,
            })
            .await;
            return output;
        }
        HeuristicOutcome::Match(intent, confidence) => {
            let output = ClassifierOutput {
                intent,
                confidence,
                reasoning: "heuristic trivial prefix match".to_string(),
                signals: vec![],
                suggested_action: None,
                latency_ms: 0,
                source: ClassificationSource::Heuristic,
                fallback: false,
            };
            let _ = append(TelemetryRecord {
                ts: chrono::Utc::now(),
                intent: output.intent,
                confidence: output.confidence,
                source: output.source,
                latency_ms: output.latency_ms,
                prompt_hash: prompt_hash_hex(&input.prompt),
                fallback: output.fallback,
            })
            .await;
            return output;
        }
        HeuristicOutcome::Indeterminate => {}
    }

    // c) cache lookup
    let key = cache::cache_key(&input.prompt);
    {
        let mut cache_guard = cache.lock().await;
        if let Some(cached) = cache_guard.get(&key) {
            let mut cached = cached.clone();
            cached.source = ClassificationSource::Cache;
            cached.latency_ms = 0;
            drop(cache_guard);
            let _ = append(TelemetryRecord {
                ts: chrono::Utc::now(),
                intent: cached.intent,
                confidence: cached.confidence,
                source: cached.source,
                latency_ms: cached.latency_ms,
                prompt_hash: prompt_hash_hex(&input.prompt),
                fallback: cached.fallback,
            })
            .await;
            return cached;
        }
    }

    // d) LLM call
    let start = Instant::now();
    let raw_result = backend.classify_llm(&input).await;
    let latency_ms = start.elapsed().as_millis() as u32;

    match raw_result {
        Ok(raw) => {
            match parse_llm_response(&raw.raw_json) {
                Ok(mut output) => {
                    output.latency_ms = latency_ms;
                    output.source = ClassificationSource::Llm;
                    // f) write cache
                    {
                        let mut cache_guard = cache.lock().await;
                        cache_guard.put(key, output.clone());
                    }
                    // g) telemetry
                    let _ = append(TelemetryRecord {
                        ts: chrono::Utc::now(),
                        intent: output.intent,
                        confidence: output.confidence,
                        source: output.source,
                        latency_ms: output.latency_ms,
                        prompt_hash: prompt_hash_hex(&input.prompt),
                        fallback: output.fallback,
                    })
                    .await;
                    output
                }
                Err(_) => {
                    let mut output = fallback_from_heuristic(&input);
                    output.latency_ms = latency_ms;
                    output.reasoning =
                        "kimi response malformed; heuristic default to small".to_string();
                    let _ = append(TelemetryRecord {
                        ts: chrono::Utc::now(),
                        intent: output.intent,
                        confidence: output.confidence,
                        source: output.source,
                        latency_ms: output.latency_ms,
                        prompt_hash: prompt_hash_hex(&input.prompt),
                        fallback: output.fallback,
                    })
                    .await;
                    output
                }
            }
        }
        Err(_) => {
            let mut output = fallback_from_heuristic(&input);
            output.latency_ms = latency_ms;
            output.reasoning = "kimi unreachable; heuristic default".to_string();
            let _ = append(TelemetryRecord {
                ts: chrono::Utc::now(),
                intent: output.intent,
                confidence: output.confidence,
                source: output.source,
                latency_ms: output.latency_ms,
                prompt_hash: prompt_hash_hex(&input.prompt),
                fallback: output.fallback,
            })
            .await;
            output
        }
    }
}

fn fallback_from_heuristic(input: &ClassifierInput) -> ClassifierOutput {
    let heuristic = heuristic_classify(&input.prompt);
    match heuristic {
        HeuristicOutcome::Match(intent, confidence) => {
            let confidence = (confidence - 0.2).max(0.0);
            ClassifierOutput {
                intent,
                confidence,
                reasoning: "heuristic fallback after LLM failure".to_string(),
                signals: vec![],
                suggested_action: None,
                latency_ms: 0,
                source: ClassificationSource::Heuristic,
                fallback: true,
            }
        }
        _ => ClassifierOutput {
            intent: Intent::Small,
            confidence: 0.5,
            reasoning: "kimi unreachable; heuristic default".to_string(),
            signals: vec![],
            suggested_action: None,
            latency_ms: 0,
            source: ClassificationSource::Heuristic,
            fallback: true,
        },
    }
}

#[derive(Debug, serde::Deserialize)]
struct LlmResponseSchema {
    intent: String,
    confidence: f32,
    reasoning: String,
    signals: Vec<String>,
    suggested_action: Option<String>,
}

fn parse_llm_response(raw_json: &str) -> anyhow::Result<ClassifierOutput> {
    let raw: LlmResponseSchema = serde_json::from_str(raw_json.trim())?;
    let intent = parse_intent(&raw.intent).ok_or_else(|| anyhow::anyhow!("unknown intent"))?;
    let signals = raw.signals.iter().filter_map(|s| parse_signal(s)).collect();
    Ok(ClassifierOutput {
        intent,
        confidence: raw.confidence.clamp(0.0, 1.0),
        reasoning: raw.reasoning,
        signals,
        suggested_action: raw.suggested_action,
        latency_ms: 0,
        source: ClassificationSource::Llm,
        fallback: false,
    })
}

fn parse_intent(raw: &str) -> Option<Intent> {
    match raw.to_lowercase().trim() {
        "trivial" => Some(Intent::Trivial),
        "small" => Some(Intent::Small),
        "medium" => Some(Intent::Medium),
        "large" => Some(Intent::Large),
        _ => None,
    }
}

fn parse_signal(raw: &str) -> Option<Signal> {
    match raw.to_lowercase().trim() {
        "multi_file" => Some(Signal::MultiFile),
        "security_sensitive" => Some(Signal::SecuritySensitive),
        "single_function" => Some(Signal::SingleFunction),
        "lookup" => Some(Signal::Lookup),
        "destructive_action" => Some(Signal::DestructiveAction),
        "new_feature" => Some(Signal::NewFeature),
        "bug_fix" => Some(Signal::BugFix),
        "refactor" => Some(Signal::Refactor),
        "docs_only" => Some(Signal::DocsOnly),
        "tests_only" => Some(Signal::TestsOnly),
        _ => None,
    }
}
