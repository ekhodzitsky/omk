use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single message in the conversation log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub ts: DateTime<Utc>,
    pub role: String,
    pub text: String,
}

/// Append-only conversation store backed by a JSONL file.
#[derive(Debug)]
pub struct ConversationLog {
    path: PathBuf,
}

impl ConversationLog {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    pub fn append_user(&mut self, text: &str) -> Result<()> {
        let msg = Message {
            ts: Utc::now(),
            role: "user".to_string(),
            text: text.to_string(),
        };
        self.append(&msg)
    }

    pub fn append_assistant(&mut self, text: &str) -> Result<()> {
        let msg = Message {
            ts: Utc::now(),
            role: "assistant".to_string(),
            text: text.to_string(),
        };
        self.append(&msg)
    }

    fn append(&mut self, msg: &Message) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(msg)? + "\n";
        file.write_all(line.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    pub fn read_all(&self) -> Result<Vec<Message>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let msg: Message = serde_json::from_str(&line)?;
            messages.push(msg);
        }
        Ok(messages)
    }
}

/// Durable metadata for a shell session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub project_root: String,
    pub last_activity: DateTime<Utc>,
    pub theme: String,
    pub schema_version: i32,
}

impl SessionMeta {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let meta: Self = serde_json::from_str(&contents)?;
        Ok(meta)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn conversation_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("conv.jsonl");
        let mut log = ConversationLog::open(&path).unwrap();
        log.append_user("hello").unwrap();
        log.append_assistant("hi").unwrap();

        let msgs = log.read_all().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].text, "hello");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].text, "hi");
    }

    #[test]
    fn meta_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("meta.json");
        let meta = SessionMeta {
            session_id: "o7k_abc12345".to_string(),
            started_at: Utc::now(),
            project_root: "/tmp".to_string(),
            last_activity: Utc::now(),
            theme: "dark".to_string(),
            schema_version: 1,
        };
        meta.save(&path).unwrap();
        let loaded = SessionMeta::load(&path).unwrap();
        assert_eq!(loaded.session_id, meta.session_id);
        assert_eq!(loaded.theme, meta.theme);
    }
}
