// Ralph progress display.
use crate::runtime::state::{RalphState, StoryStatus};
use tracing::info;

pub(crate) fn print_progress(state: &RalphState) {
    let verified = state
        .prd
        .user_stories
        .iter()
        .filter(|s| matches!(s.status, StoryStatus::Verified))
        .count();
    let failed = state
        .prd
        .user_stories
        .iter()
        .filter(|s| matches!(s.status, StoryStatus::Failed))
        .count();
    let total = state.prd.user_stories.len();

    info!(
        "🔄 Ralph: {}/{} stories verified, {} failed (iteration {}/{})",
        verified, total, failed, state.iteration, state.max_iterations
    );
    for story in &state.prd.user_stories {
        let icon = match story.status {
            StoryStatus::Verified => "✓",
            StoryStatus::Failed => "✗",
            StoryStatus::InProgress => "▶",
            StoryStatus::Implemented => "◐",
            StoryStatus::NotStarted => "○",
        };
        info!("   {} {}", icon, story.id);
    }
}
