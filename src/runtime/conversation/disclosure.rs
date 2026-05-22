use crate::runtime::classifier::Intent;
use crate::runtime::conversation::bus::ActiveMode;

pub fn format_disclosure(
    intent: Intent,
    _target_mode: ActiveMode,
    task_summary: Option<&str>,
) -> Option<String> {
    match intent {
        Intent::Trivial => None,
        Intent::Small => {
            let summary = task_summary.unwrap_or("unspecified");
            Some(format!("→ small edit: {summary}"))
        }
        Intent::Medium => {
            let summary = task_summary.unwrap_or("sequential workers");
            Some(format!("→ medium task: {summary}"))
        }
        Intent::Large => {
            let summary = task_summary.unwrap_or("launching goal-mode (slice PR will be created)");
            Some(format!("→ large feature: {summary}"))
        }
    }
}
