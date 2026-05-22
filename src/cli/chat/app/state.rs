use std::path::PathBuf;

use crate::cli::chat::persistence::{ConversationLog, SessionMeta};

/// Visibility state of the right-hand engine pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneState {
    Collapsed,
    Compact,
    Expanded,
}

/// High-level action returned by the event handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
    Redraw,
}

/// Persistent session container.
#[derive(Debug)]
pub struct SessionState {
    pub meta: SessionMeta,
    pub conversation: ConversationLog,
}

pub fn default_state_dir(session_id: &str) -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("state")
        .join("omk")
        .join("sessions")
        .join(session_id)
}

pub fn default_config_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("omk")
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn check_tab_hint(config_dir: &std::path::Path) -> bool {
    let path = config_dir.join("seen.json");
    if !path.exists() {
        return false;
    }
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    val.get("tab_hint")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}
