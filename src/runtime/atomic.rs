//! Atomic file writes using temp file + rename pattern.
//!
//! This ensures readers never see partially-written files,
//! even if the process crashes mid-write.

use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::debug;

/// Write `content` to `path` atomically.
///
/// 1. Writes to a temp file next to `path`
/// 2. Flushes and syncs the temp file
/// 3. Renames temp file to `path`
///
/// On Windows this requires `fs::rename` to overwrite, which is atomic.
/// On Unix, `rename` is always atomic.
pub async fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().unwrap_or_default();
    let tmp_name = format!(
        ".{}.tmp.{}",
        file_name.to_string_lossy(),
        uuid::Uuid::new_v4()
    );
    let tmp_path = dir.join(&tmp_name);

    debug!(tmp = %tmp_path.display(), target = %path.display(), "Atomic write start");

    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_private_create_mode(&mut options);

    let mut file = options
        .open(&tmp_path)
        .await
        .with_context(|| format!("Failed to create temp file: {}", tmp_path.display()))?;
    set_private_file_permissions(&file, &tmp_path).await?;

    file.write_all(content).await?;
    file.flush().await?;

    // sync_data ensures the OS has flushed the file to disk before rename
    let std_file = file.into_std().await;
    std_file.sync_data()?;
    drop(std_file);

    fs::rename(&tmp_path, path).await.with_context(|| {
        format!(
            "Failed to rename {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    debug!(target = %path.display(), "Atomic write complete");
    Ok(())
}

/// Append raw bytes to a file.
///
/// Opens the file in append mode (creating if necessary) and writes
/// the content. This is not atomic across processes, but it is
/// sufficient for append-only logs when a single writer is guaranteed.
pub async fn atomic_append(path: &Path, content: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.create(true).append(true);
    set_private_create_mode(&mut options);

    let mut file = options
        .open(path)
        .await
        .with_context(|| format!("Failed to open file for append: {}", path.display()))?;
    set_private_file_permissions(&file, path).await?;
    file.write_all(content).await?;
    file.flush().await?;
    Ok(())
}

#[cfg(unix)]
fn set_private_create_mode(options: &mut fs::OpenOptions) {
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_private_create_mode(_options: &mut fs::OpenOptions) {}

#[cfg(unix)]
async fn set_private_file_permissions(file: &fs::File, path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .await
        .with_context(|| format!("Failed to harden file permissions: {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
async fn set_private_file_permissions(_file: &fs::File, _path: &Path) -> Result<()> {
    Ok(())
}

/// Append a line to a JSONL file atomically-ish.
///
/// For true multi-process safety, use a lock file. This function
/// uses atomic_write on a copy for critical state files.
pub async fn atomic_append_jsonl(path: &Path, line: &str) -> Result<()> {
    let mut content = Vec::new();

    if path.exists() {
        content = fs::read(path).await.unwrap_or_default();
    }

    if !content.is_empty() && !content.ends_with(b"\n") {
        content.push(b'\n');
    }
    content.extend_from_slice(line.as_bytes());
    content.push(b'\n');

    atomic_write(path, &content).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_atomic_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.json");

        atomic_write(&path, b"hello world").await.unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_atomic_append_jsonl() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.jsonl");

        atomic_append_jsonl(&path, r#"{"id":"1"}"#).await.unwrap();
        atomic_append_jsonl(&path, r#"{"id":"2"}"#).await.unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"id":"1"}"#);
        assert_eq!(lines[1], r#"{"id":"2"}"#);
    }
}
