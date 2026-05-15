#![allow(dead_code)]

mod api;
mod artifact;
mod execution;
mod provider;
mod synthesis;

#[cfg(test)]
mod tests;

pub use api::{ask_all, ask_providers, ask_single};
pub use artifact::{artifact_dir, artifact_path, save_artifact, save_artifact_to};
pub use execution::{poll_outbox, run_advisor_direct};
pub use provider::{
    ALL_PROVIDERS, available_providers, is_known_provider, is_provider_installed, provider_command,
};
pub use synthesis::{build_synthesis_prompt, synthesize};
