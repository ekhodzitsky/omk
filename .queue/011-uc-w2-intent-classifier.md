---
id: 011
title: UNIFIED_CHAT W2 — intent classifier (Kimi-backed + heuristic prefilter)
status: todo
branch: feat/intent-classifier
worktree: .worktrees/unified-chat-W2-classifier
blocked_by: []
merge_after: []
size: large
batch: unified-chat-wave-1
pr: null
notes: Spec §13 W2. Per CO-4, system prompt lives at src/runtime/classifier/system_prompt.rs NOT src/llm/classifier_prompt.rs. Telemetry retain default 30 days per D5.
---

# Prompt

Ты работаешь в worktree `.worktrees/unified-chat-W2-classifier` на ветке feat/intent-classifier. Workstream: W2 — Intent classifier.

Standalone, никакого роутера, никакого исполнения. Принимаешь prompt — возвращаешь (intent, confidence, reasoning, signals, latency_ms).

==================================================================
АБСОЛЮТНЫЕ ЗАПРЕТЫ
==================================================================

§12 ANTI-GOALS — все 10. Особенно:
2. No vendor lock-in beyond Kimi — Kimi единственный writer для классификации.
4. No telemetry transmission. Telemetry — local-only (см. D5: retain 30/90d).
7. No stub commands. /classify должна реально классифицировать.

§14 COORDINATION:
- ТЫ МОЖЕШЬ ИЗМЕНЯТЬ ТОЛЬКО:
    src/runtime/classifier/   (новый, создаёшь)
    tests/classifier_test.rs
- CO-4: spec §13 W2 говорит про src/llm/classifier_prompt.rs — это РЕДИРЕКТ. Из-за in-flight рефакторов в src/llm/ system-prompt template живёт в src/runtime/classifier/system_prompt.rs. Не лезь в src/llm/.
- ТЫ НЕ ТРОГАЕШЬ: Cargo.toml, src/lib.rs, src/main.rs, src/cli/mod.rs, src/runtime/mod.rs (orchestrator зарегистрирует pub mod classifier;), docs/UNIFIED_CHAT*.md.

==================================================================
ЗАДАЧА
==================================================================

1. CLASSIFIER API:
        pub struct ClassifierInput {
            pub prompt: String,
            pub recent_conversation: Vec<ConversationTurn>,
            pub project_root: PathBuf,
        }
        pub struct ClassifierOutput {
            pub intent: Intent,
            pub confidence: f32,
            pub reasoning: String,
            pub signals: Vec<Signal>,
            pub suggested_action: Option<String>,
            pub latency_ms: u32,
            pub source: ClassificationSource,
            pub fallback: bool,
        }
        pub enum Intent { Trivial, Small, Medium, Large }
        pub enum Signal { MultiFile, SecuritySensitive, SingleFunction, Lookup,
                          DestructiveAction, NewFeature, BugFix, /* extend conservatively */ }
        pub enum ClassificationSource { Heuristic, Llm, Cache }

        pub async fn classify(input: ClassifierInput) -> ClassifierOutput;

2. HEURISTIC PREFILTER (no LLM call):
   src/runtime/classifier/heuristic.rs:
   - prompt.len() <= 80 AND starts with one of [
        "what is", "explain", "show", "how does", "where is",
        "does", "define", "summarise", "summary of"
     ] → Intent::Trivial, confidence=1.0, source=Heuristic
   - prompt starts with '/' → Err(ClassifierError::SlashCommand) (caller дёргает W5)
   - empty / whitespace → Err(ClassifierError::Empty)
   - всё остальное → LLM
   Case-insensitive по unicode.

3. LLM CLASSIFIER:
   Используй существующий src/llm/client.rs WireLlmClient если интерфейс подходит. Иначе trait LlmClassifierBackend + impl WireLlmClient (production) + impl MockLlmClassifier (tests).

   System prompt в src/runtime/classifier/system_prompt.rs:
        pub const CLASSIFIER_SYSTEM_PROMPT: &str = r#"
        You are an intent classifier for OMK, a Kimi-native CLI. Given a user prompt and
        optional recent conversation context, classify the intent as exactly one of:
          - trivial  : Q&A about existing code, no edits.
          - small    : single-file or single-symbol edit; no architecture.
          - medium   : multi-step but bounded; new tests, new functions.
          - large    : new feature, multi-file architectural change, security implications,
                       or PR-worthy delivery.
        Output a JSON object with:
          { "intent": "trivial"|"small"|"medium"|"large",
            "confidence": <0.0-1.0>,
            "reasoning": "<one sentence>",
            "signals": [<tags>],
            "suggested_action": "<optional one-line hint>" }
        Do NOT include any markdown, prose, or commentary outside the JSON.
        "#;

   Парсинг: serde_json::from_str. На malformed JSON — fallback в heuristic (confidence -0.2, signals.push(malformed sentinel)).

4. CACHE: LRU 50, per-session. Key: normalized prompt (trim, collapse ws, lowercase). Используй крейт `lru` (если нет — запроси). Cache hit: latency_ms ≈ 1-5, source=Cache.

5. FALLBACK MODE: на failure LLM (timeout/transport/parse-after-retry):
   - heuristic-результат, fallback=true
   - если heuristic был бы None — default Intent::Small, confidence=0.5, reasoning="kimi unreachable; heuristic default"
   - НИКОГДА не Large в fallback.

6. TELEMETRY:
   После каждой классификации append в ~/.local/state/omk/telemetry.jsonl:
        { "ts": "...", "intent": "small", "confidence": 0.82,
          "source": "Llm", "latency_ms": 287, "prompt_hash": "<sha256-8>",
          "fallback": false }
   NEVER store сам prompt — только хэш.
   Retention (D5): rotate file при entries старше 30 дней.
   telemetry::compact_if_stale(path, retain_days=30) вызывается в начале classify().

7. CONFIDENCE THRESHOLDS — const'ы, НЕ применяешь (W3 работа):
        pub const CONFIDENCE_AUTO_EXECUTE: f32 = 0.85;
        pub const CONFIDENCE_INLINE_DISCLAIMER: f32 = 0.65;
        pub const CONFIDENCE_FIRST_PROMPT: f32 = 0.85;  // D3

8. /classify SLASH PARSER:
   src/runtime/classifier/cli.rs — handle_classify_command. Записывает событие в engine-events.jsonl даже если W3/W4 не готовы (свой минимальный line-atomic JSONL).

==================================================================
РАЗВЕДКА
==================================================================

1. src/llm/client.rs — есть ли уже trait LlmClient? Какая сигнатура chat-completion?
2. src/wire/protocol.rs — Wire Protocol message types.
3. src/runtime/events/writer.rs — line-atomic JSONL pattern.
4. Cargo.toml — есть ли lru, serde_json, tokio, anyhow, sha2?

==================================================================
ТЕСТЫ
==================================================================

tests/classifier_test.rs:
- 50 hand-curated prompts → assert >85% agreement с hand-labels (const PROMPTS_LABELED: [(&str, Intent); 50]).
- LLM mock: MockLlmClassifier с заранее заданными ответами по prompt hash.
- test_cache_hit_latency_under_5ms
- test_heuristic_latency_under_1ms
- test_llm_fallback_returns_heuristic_not_large
- SimulatedLlmFailure → assert не Large, fallback=true.

Минимум 8 тестов.

==================================================================
СКЕЛЕТ
==================================================================

src/runtime/classifier/mod.rs:
    pub mod heuristic;
    pub mod llm_backend;
    pub mod cache;
    pub mod system_prompt;
    pub mod telemetry;
    pub mod cli;
    mod types;
    mod errors;
    pub use types::*;
    pub use errors::ClassifierError;
    pub use cli::handle_classify_command;
    pub const CONFIDENCE_AUTO_EXECUTE: f32 = 0.85;
    pub const CONFIDENCE_INLINE_DISCLAIMER: f32 = 0.65;
    pub const CONFIDENCE_FIRST_PROMPT: f32 = 0.85;
    pub async fn classify(input: ClassifierInput) -> ClassifierOutput;

==================================================================
SUCCESS CRITERIA
==================================================================

- cargo build --workspace зелено
- cargo test --test classifier_test зелено, ≥85% agreement
- cargo clippy --all-targets -- -D warnings зелено
- cargo fmt --check зелено
- git status: только src/runtime/classifier/** и tests/classifier_test.rs
- PR title: "feat(classifier): intent classifier with heuristic + Kimi backend (W2)"
- PR body: запрошенные deps (lru), запрос на pub mod classifier; в src/runtime/mod.rs, agreement score, latency p99.

==================================================================
СТОП-ТРИГГЕРЫ
==================================================================

- LlmClient trait не существует / шейп не подходит — STOP.
- Нет lru — STOP, запроси.
- Hand-curated 50-prompt не даёт 85% — STOP, не натягивай labels.

НЕ ПЕРЕСМАТРИВАЙ §5, §13, §14.

Начинай с разведки.
