// Autopilot state machine — 6-phase pipeline
#![allow(dead_code)] // API surface for future features (run_command, project type detection)

mod cli;
mod engine;
mod helpers;
mod types;

pub use cli::*;
pub use engine::Autopilot;
pub use types::*;
