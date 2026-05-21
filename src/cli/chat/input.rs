use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Current input modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal text insertion (default).
    Text,
    /// Slash-command mode (triggered by leading '/').
    Command,
}

/// A portable key event used by the shell event loop.
///
/// Mirrors the subset of crossterm keys we care about so that tests do not
/// need to depend on crossterm directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    BackTab,
    Up,
    Down,
    PageUp,
    PageDown,
    Esc,
    Backspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl KeyModifiers {
    pub const fn none() -> Self {
        Self {
            shift: false,
            control: false,
            alt: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// Events fed into the App state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatEvent {
    Key(KeyEvent),
    Tick,
}

/// In-memory input history with optional persistence.
#[derive(Debug, Clone)]
pub struct InputHistory {
    entries: VecDeque<String>,
    path: Option<PathBuf>,
    max_size: usize,
    cursor: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct HistoryEntry {
    text: String,
}

impl InputHistory {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut history = Self {
            entries: VecDeque::new(),
            path,
            max_size: 100,
            cursor: None,
        };
        if let Some(ref p) = history.path.clone() {
            history.load_from_disk(p);
        }
        history
    }

    fn load_from_disk(&mut self, path: &Path) {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return;
        };
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
                self.entries.push_back(entry.text);
            }
        }
        while self.entries.len() > self.max_size {
            self.entries.pop_front();
        }
    }

    /// Push a new prompt into history.
    pub fn push(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        self.entries.push_back(text);
        while self.entries.len() > self.max_size {
            self.entries.pop_front();
        }
        self.cursor = None;
        self.persist();
    }

    /// Move cursor up (older entry). Returns the text to display.
    pub fn navigate_up(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        self.cursor = Some(match self.cursor {
            Some(c) => (c + 1).min(self.entries.len() - 1),
            None => 0,
        });
        self.current_text()
    }

    /// Move cursor down (newer entry). Returns the text to display.
    pub fn navigate_down(&mut self) -> Option<&str> {
        let c = self.cursor?;
        if c == 0 {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(c - 1);
        self.current_text()
    }

    fn current_text(&self) -> Option<&str> {
        let c = self.cursor?;
        let idx = self.entries.len().saturating_sub(1 + c);
        self.entries.get(idx).map(|s| s.as_str())
    }

    fn persist(&self) {
        let Some(ref path) = self.path else {
            return;
        };
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
        else {
            return;
        };
        use std::io::Write;
        for entry in &self.entries {
            let line = match serde_json::to_string(&HistoryEntry {
                text: entry.clone(),
            }) {
                Ok(l) => l,
                Err(_) => continue,
            };
            if file.write_all(line.as_bytes()).is_err() {
                return;
            }
            if file.write_all(b"\n").is_err() {
                return;
            }
        }
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_push_and_navigate() {
        let mut h = InputHistory::new(None);
        h.push("first".to_string());
        h.push("second".to_string());
        assert_eq!(h.navigate_up(), Some("second"));
        assert_eq!(h.navigate_up(), Some("first"));
        assert_eq!(h.navigate_down(), Some("second"));
        assert_eq!(h.navigate_down(), None);
    }
}
