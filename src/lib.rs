#![warn(clippy::await_holding_lock)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::wildcard_imports)]
#![warn(clippy::unused_async)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub mod agents;
pub mod cli;
pub mod cost;
pub mod error;
pub mod kimi_native;
pub mod marketplace;
pub mod mcp;
pub mod notifications;
pub mod runtime;
pub mod skills;
#[doc(hidden)]
pub mod test_helpers;
pub mod vis;
pub mod wire;
