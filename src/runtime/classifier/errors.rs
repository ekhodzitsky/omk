#[derive(thiserror::Error, Debug)]
pub enum ClassifierError {
    #[error("input is a slash command (not classifiable)")]
    SlashCommand,
    #[error("input is empty or whitespace")]
    Empty,
    #[error("LLM backend error: {0}")]
    LlmBackend(#[source] anyhow::Error),
    #[error("LLM response malformed: {0}")]
    MalformedLlmResponse(String),
}
