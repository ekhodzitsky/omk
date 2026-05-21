use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ClassifierInput {
    pub prompt: String,
    pub recent_conversation: Vec<ConversationTurn>,
    pub project_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ConversationTurn {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Trivial,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    MultiFile,
    SecuritySensitive,
    SingleFunction,
    Lookup,
    DestructiveAction,
    NewFeature,
    BugFix,
    Refactor,
    DocsOnly,
    TestsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationSource {
    Heuristic,
    Llm,
    Cache,
}
