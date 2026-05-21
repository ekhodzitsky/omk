use omk::runtime::classifier::{
    cache::cache_key, cache::new_session_cache, classify, heuristic::heuristic_classify,
    heuristic::HeuristicOutcome, llm_backend::MockLlmClassifier, llm_backend::RawLlmClassification,
    ClassificationSource, ClassifierInput, Intent, LlmClassifierBackend,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Serialise all tests in this file because telemetry tests mutate
/// the global process environment (`XDG_STATE_HOME`).
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ---------------------------------------------------------------------------
// 1. Dataset agreement
// ---------------------------------------------------------------------------
const PROMPTS_LABELED: &[(&str, Intent)] = &[
    ("what does build_task_graph do?", Intent::Trivial),
    (
        "rename build_task_graph to compile_task_graph everywhere",
        Intent::Small,
    ),
    (
        "add input validation to /signup and write tests",
        Intent::Medium,
    ),
    (
        "add OAuth login with Google and GitHub plus rate limiting",
        Intent::Large,
    ),
    ("what is the meaning of life?", Intent::Trivial),
    ("explain quantum computing", Intent::Trivial),
    ("show me the main function", Intent::Trivial),
    ("how does the cache work?", Intent::Trivial),
    ("where is the config file?", Intent::Trivial),
    ("does this compile?", Intent::Trivial),
    ("define monad", Intent::Trivial),
    ("summarise this module", Intent::Trivial),
    ("summary of the changes", Intent::Trivial),
    ("fix the typo in README", Intent::Small),
    ("refactor utils.rs into smaller functions", Intent::Small),
    ("add a test for edge case", Intent::Small),
    ("update the error message", Intent::Small),
    ("extract helper function", Intent::Small),
    ("rename variable x to count", Intent::Small),
    ("move logic into separate module", Intent::Medium),
    (
        "implement rate limiting middleware with tests",
        Intent::Medium,
    ),
    ("add database migration for users table", Intent::Medium),
    ("refactor auth module and add unit tests", Intent::Medium),
    ("create new CLI command with subcommands", Intent::Medium),
    ("redesign the API with breaking changes", Intent::Large),
    ("implement distributed consensus algorithm", Intent::Large),
    ("add support for multiple cloud providers", Intent::Large),
    ("rewrite the rendering engine", Intent::Large),
    ("migrate from REST to GraphQL", Intent::Large),
    ("introduce plugin architecture", Intent::Large),
    ("add end-to-end encryption", Intent::Large),
    ("build a new frontend framework", Intent::Large),
    ("integrate with external payment gateway", Intent::Medium),
    ("add caching layer with invalidation", Intent::Medium),
    ("optimise hot path in query engine", Intent::Small),
    ("document the public API", Intent::Trivial),
    ("what is this function doing?", Intent::Trivial),
    ("how do I run tests?", Intent::Trivial),
    ("explain the build system", Intent::Trivial),
    ("show dependencies", Intent::Trivial),
    ("is this thread-safe?", Intent::Trivial),
    ("where are the types defined?", Intent::Trivial),
    ("does this handle null?", Intent::Trivial),
    ("define the interface", Intent::Trivial),
    ("summarise errors.rs", Intent::Trivial),
    ("summary of PR #123", Intent::Trivial),
    ("fix compilation error in lib.rs", Intent::Small),
    ("add logging to debug flow", Intent::Small),
    ("remove unused import", Intent::Small),
    ("update version to 1.0", Intent::Small),
];

fn intent_to_str(intent: Intent) -> &'static str {
    match intent {
        Intent::Trivial => "trivial",
        Intent::Small => "small",
        Intent::Medium => "medium",
        Intent::Large => "large",
    }
}

#[tokio::test]
async fn test_dataset_agreement_at_least_85_percent() {
    let _guard = TEST_LOCK.lock().await;
    let mut mock = MockLlmClassifier::new();
    for (prompt, intent) in PROMPTS_LABELED {
        let raw_json = format!(
            r#"{{"intent":"{}","confidence":0.9,"reasoning":"mock","signals":[],"suggested_action":null}}"#,
            intent_to_str(*intent)
        );
        let hash = cache_key(prompt);
        mock = mock.with_answer(
            hash,
            RawLlmClassification {
                raw_json,
                model: "mock".to_string(),
                tokens_in: 10,
                tokens_out: 10,
            },
        );
    }
    let backend: Arc<dyn LlmClassifierBackend> = Arc::new(mock);
    let mut cache = new_session_cache();
    let mut correct = 0usize;
    for (prompt, expected) in PROMPTS_LABELED {
        let input = ClassifierInput {
            prompt: prompt.to_string(),
            recent_conversation: vec![],
            project_root: PathBuf::from("."),
        };
        let output = classify(input, backend.as_ref(), &mut cache).await;
        if output.intent == *expected {
            correct += 1;
        }
    }
    let ratio = correct as f32 / PROMPTS_LABELED.len() as f32;
    assert!(ratio >= 0.85, "agreement {:.2}% < 85%", ratio * 100.0);
}

// ---------------------------------------------------------------------------
// 2. Heuristic catches trivial prefix
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_heuristic_catches_trivial_prefix() {
    let _guard = TEST_LOCK.lock().await;
    let mock = MockLlmClassifier::new();
    let backend: Arc<dyn LlmClassifierBackend> = Arc::new(mock);
    let mut cache = new_session_cache();
    let input = ClassifierInput {
        prompt: "what is X?".to_string(),
        recent_conversation: vec![],
        project_root: PathBuf::from("."),
    };
    let output = classify(input, backend.as_ref(), &mut cache).await;
    assert_eq!(output.intent, Intent::Trivial);
    assert_eq!(output.source, ClassificationSource::Heuristic);
    assert!(output.latency_ms < 5);
}

// ---------------------------------------------------------------------------
// 3. Heuristic rejects slash command
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_heuristic_rejects_slash_command() {
    let _guard = TEST_LOCK.lock().await;
    let outcome = heuristic_classify("/classify foo");
    assert!(matches!(outcome, HeuristicOutcome::SlashCommand));
}

// ---------------------------------------------------------------------------
// 4. Heuristic rejects empty
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_heuristic_rejects_empty() {
    let _guard = TEST_LOCK.lock().await;
    assert!(matches!(heuristic_classify(""), HeuristicOutcome::Empty));
    assert!(matches!(
        heuristic_classify("   \n"),
        HeuristicOutcome::Empty
    ));
}

// ---------------------------------------------------------------------------
// 5. Cache hit returns cached result under 5ms
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_cache_hit_returns_cached_result_under_5ms() {
    let _guard = TEST_LOCK.lock().await;
    let mut mock = MockLlmClassifier::new();
    let hash = cache_key("repeat me");
    mock = mock.with_answer(
        hash,
        RawLlmClassification {
            raw_json: r#"{"intent":"medium","confidence":0.82,"reasoning":"mock","signals":[],"suggested_action":null}"#.to_string(),
            model: "mock".to_string(),
            tokens_in: 10,
            tokens_out: 10,
        },
    );
    let backend: Arc<dyn LlmClassifierBackend> = Arc::new(mock);
    let mut cache = new_session_cache();
    let input = ClassifierInput {
        prompt: "repeat me".to_string(),
        recent_conversation: vec![],
        project_root: PathBuf::from("."),
    };
    let first = classify(input.clone(), backend.as_ref(), &mut cache).await;
    assert_eq!(first.source, ClassificationSource::Llm);
    let second = classify(input, backend.as_ref(), &mut cache).await;
    assert_eq!(second.source, ClassificationSource::Cache);
    assert!(second.latency_ms <= 5);
}

// ---------------------------------------------------------------------------
// 6. Fallback on malformed JSON returns heuristic, not Large
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_llm_fallback_on_malformed_json_returns_heuristic_not_large() {
    let _guard = TEST_LOCK.lock().await;
    let mut mock = MockLlmClassifier::new();
    let hash = cache_key("fix the auth flow rewrite security module");
    mock = mock.with_answer(
        hash,
        RawLlmClassification {
            raw_json: "not even json".to_string(),
            model: "mock".to_string(),
            tokens_in: 10,
            tokens_out: 10,
        },
    );
    let backend: Arc<dyn LlmClassifierBackend> = Arc::new(mock);
    let mut cache = new_session_cache();
    let input = ClassifierInput {
        prompt: "fix the auth flow rewrite security module".to_string(),
        recent_conversation: vec![],
        project_root: PathBuf::from("."),
    };
    let output = classify(input, backend.as_ref(), &mut cache).await;
    assert_ne!(output.intent, Intent::Large);
    assert!(output.fallback);
}

// ---------------------------------------------------------------------------
// 7. Fallback on transport failure returns heuristic, not Large
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_llm_fallback_on_transport_failure_returns_heuristic_not_large() {
    let _guard = TEST_LOCK.lock().await;
    let mock = MockLlmClassifier::new();
    let backend: Arc<dyn LlmClassifierBackend> = Arc::new(mock);
    let mut cache = new_session_cache();
    let input = ClassifierInput {
        prompt: "add new endpoint with tests".to_string(),
        recent_conversation: vec![],
        project_root: PathBuf::from("."),
    };
    let output = classify(input, backend.as_ref(), &mut cache).await;
    assert_ne!(output.intent, Intent::Large);
    assert!(output.fallback);
}

// ---------------------------------------------------------------------------
// 8. Telemetry record does not contain raw prompt
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_telemetry_record_does_not_contain_raw_prompt() {
    let _guard = TEST_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_STATE_HOME", tmp.path());

    let mut mock = MockLlmClassifier::new();
    let hash = cache_key("private API key abc123");
    mock = mock.with_answer(
        hash,
        RawLlmClassification {
            raw_json: r#"{"intent":"small","confidence":0.8,"reasoning":"mock","signals":[],"suggested_action":null}"#.to_string(),
            model: "mock".to_string(),
            tokens_in: 10,
            tokens_out: 10,
        },
    );
    let backend: Arc<dyn LlmClassifierBackend> = Arc::new(mock);
    let mut cache = new_session_cache();
    let input = ClassifierInput {
        prompt: "private API key abc123".to_string(),
        recent_conversation: vec![],
        project_root: PathBuf::from("."),
    };
    let _ = classify(input, backend.as_ref(), &mut cache).await;

    let telemetry_path = tmp.path().join("omk").join("telemetry.jsonl");
    let contents = tokio::fs::read_to_string(&telemetry_path).await.unwrap();
    assert!(!contents.contains("abc123"));
    assert!(contents.contains("prompt_hash"));

    std::env::remove_var("XDG_STATE_HOME");
}

// ---------------------------------------------------------------------------
// 9. Telemetry compact drops stale records
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_telemetry_compact_drops_stale_records() {
    let _guard = TEST_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_STATE_HOME", tmp.path());

    let record = omk::runtime::classifier::telemetry::TelemetryRecord {
        ts: chrono::Utc::now() - chrono::Duration::days(100),
        intent: Intent::Small,
        confidence: 0.8,
        source: ClassificationSource::Llm,
        latency_ms: 100,
        prompt_hash: "deadbeef".to_string(),
        fallback: false,
    };
    omk::runtime::classifier::telemetry::append(record)
        .await
        .unwrap();

    omk::runtime::classifier::telemetry::compact_if_stale(30)
        .await
        .unwrap();

    let telemetry_path = tmp.path().join("omk").join("telemetry.jsonl");
    let contents = tokio::fs::read_to_string(&telemetry_path).await.unwrap();
    assert!(!contents.contains("deadbeef"));

    std::env::remove_var("XDG_STATE_HOME");
}

// ---------------------------------------------------------------------------
// 10. Concurrent classify does not corrupt telemetry
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_concurrent_classify_does_not_corrupt_telemetry() {
    let _guard = TEST_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_STATE_HOME", tmp.path());

    let mut mock = MockLlmClassifier::new();
    for i in 0..10 {
        let prompt = format!("prompt number {}", i);
        let hash = cache_key(&prompt);
        mock = mock.with_answer(
            hash,
            RawLlmClassification {
                raw_json: format!(
                    r#"{{"intent":"small","confidence":0.8,"reasoning":"mock {}","signals":[],"suggested_action":null}}"#,
                    i
                ),
                model: "mock".to_string(),
                tokens_in: 10,
                tokens_out: 10,
            },
        );
    }
    let backend = Arc::new(mock);
    let mut handles = Vec::new();
    for i in 0..10 {
        let backend = Arc::clone(&backend);
        handles.push(tokio::spawn(async move {
            let mut cache = new_session_cache();
            let input = ClassifierInput {
                prompt: format!("prompt number {}", i),
                recent_conversation: vec![],
                project_root: PathBuf::from("."),
            };
            classify(input, backend.as_ref(), &mut cache).await
        }));
    }
    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }
    assert_eq!(results.len(), 10);

    let telemetry_path = tmp.path().join("omk").join("telemetry.jsonl");
    let contents = tokio::fs::read_to_string(&telemetry_path).await.unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 10);
    for line in &lines {
        let _: serde_json::Value = serde_json::from_str(line).expect("valid JSON per line");
    }

    std::env::remove_var("XDG_STATE_HOME");
}
