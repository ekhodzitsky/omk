mod artifacts;
mod decompose;
mod delivery;
mod discover;
mod scaffold;

pub(crate) use decompose::{decompose_goal_for_slices, sanitize_feature_slug};
pub use discover::discover_relevant_files;
pub(crate) use scaffold::{controller_task_summary, create_goal_with_scaffold};
