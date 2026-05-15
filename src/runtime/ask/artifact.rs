use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::info;

/// Directory where ask artifacts are persisted.
pub fn artifact_dir() -> Result<PathBuf> {
    Ok(crate::runtime::config::omk_data_dir()
        .join("artifacts")
        .join("ask"))
}

/// Full path for a named artifact at a given timestamp.
pub fn artifact_path(name: &str, timestamp: &str) -> Result<PathBuf> {
    let dir = artifact_dir()?;
    Ok(dir.join(format!("{}-{name}.md", timestamp)))
}

/// Save an artifact to a specific base directory (useful for testing).
pub async fn save_artifact_to(
    base_dir: &Path,
    name: &str,
    content: &str,
    timestamp: &str,
) -> Result<PathBuf> {
    let path = base_dir.join(format!("{}-{name}.md", timestamp));
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, content).await?;
    info!(path = %path.display(), name = name, "Saved artifact");
    Ok(path)
}

/// Save an artifact to the default `.omk/artifacts/ask` directory.
pub async fn save_artifact(name: &str, content: &str, timestamp: &str) -> Result<PathBuf> {
    let dir = artifact_dir()?;
    save_artifact_to(&dir, name, content, timestamp).await
}
