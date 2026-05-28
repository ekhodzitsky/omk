#![warn(clippy::await_holding_lock)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::wildcard_imports)]
#![warn(clippy::unused_async)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub(crate) mod agents;
pub(crate) mod analysis;
pub mod cli;
pub mod cost;
pub mod error;
pub mod git;
pub mod kimi_native;
pub mod llm;
pub(crate) mod marketplace;
pub mod mcp;
pub(crate) mod notifications;
pub mod runtime;
pub(crate) mod skills;
#[cfg(test)]
pub mod test_helpers;
pub mod vis;
pub mod wire;
