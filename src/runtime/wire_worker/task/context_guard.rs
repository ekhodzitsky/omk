use tracing::warn;

use crate::wire::Request;

/// Warn when a wire message exceeds ~90 % of a typical 128 k context window.
const CONTEXT_WINDOW_WARNING_THRESHOLD: usize = 115_200;

/// Log a warning if the prompt text exceeds the context-window threshold.
pub(super) fn warn_if_prompt_exceeds_threshold(prompt: &str, worker_name: &str, task_id: &str) {
    let tokens = crate::cost::tokens::count_tokens(prompt, "gpt-4");
    if tokens > CONTEXT_WINDOW_WARNING_THRESHOLD {
        warn!(
            worker = %worker_name,
            task = %task_id,
            tokens,
            "Prompt exceeds 90% of typical context window"
        );
    }
}

/// Log a warning if a wire request exceeds the context-window threshold.
pub(super) fn warn_if_request_exceeds_threshold(
    request: &Request,
    worker_name: &str,
    task_id: &str,
) {
    let tokens = crate::cost::tokens::count_message_tokens(request, "gpt-4").unwrap_or(0);
    if tokens > CONTEXT_WINDOW_WARNING_THRESHOLD {
        warn!(
            worker = %worker_name,
            task = %task_id,
            tokens,
            "Wire request exceeds 90% of typical context window"
        );
    }
}
