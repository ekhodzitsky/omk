//! Test helpers for isolated XDG/HOME directory setup.
//!
//! Intended for use in integration tests to avoid polluting the user's
//! real home directory with test artifacts.

use std::path::PathBuf;

/// Sets up isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`,
/// `XDG_DATA_HOME`, and `XDG_CACHE_HOME` inside a temporary directory.
///
/// Returns the temp directory handle (must be kept alive for the duration
/// of the test) and a vector of environment variable tuples.
pub fn isolated_xdg_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let xdg_config = home.join(".config");
    let xdg_state = home.join(".local").join("state");
    let xdg_data = home.join(".local").join("share");
    let xdg_cache = home.join(".cache");

    std::fs::create_dir_all(&xdg_config).unwrap();
    std::fs::create_dir_all(&xdg_state).unwrap();
    std::fs::create_dir_all(&xdg_data).unwrap();
    std::fs::create_dir_all(&xdg_cache).unwrap();

    let envs = vec![
        ("HOME", home.clone()),
        ("XDG_CONFIG_HOME", xdg_config),
        ("XDG_STATE_HOME", xdg_state),
        ("XDG_DATA_HOME", xdg_data),
        ("XDG_CACHE_HOME", xdg_cache),
    ];

    (tmp, envs)
}
