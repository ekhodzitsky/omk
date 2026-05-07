use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::Path;
use tracing::info;

/// Current state schema version.
pub const CURRENT_VERSION: u32 = 1;

/// Migrate a JSON state blob to the current version if needed.
pub async fn migrate_if_needed(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = tokio::fs::read_to_string(path).await?;
    let mut value: Value = serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;

    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    if version > CURRENT_VERSION {
        bail!(
            "state version {} at {} is newer than supported {}. Please upgrade omk.",
            version,
            path.display(),
            CURRENT_VERSION
        );
    }

    if version == CURRENT_VERSION {
        return Ok(());
    }

    info!(%version, target = %CURRENT_VERSION, path = %path.display(), "Migrating state");

    // Future migrations go here:
    // if version < 2 { migrate_v1_to_v2(&mut value)?; }

    value["version"] = CURRENT_VERSION.into();

    let out = serde_json::to_vec_pretty(&value)?;
    crate::runtime::atomic::atomic_write(path, &out).await?;

    info!(path = %path.display(), "Migration complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrate_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        migrate_if_needed(&path).await.unwrap();
    }

    #[tokio::test]
    async fn test_migrate_current_version() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        let data = serde_json::json!({"version": CURRENT_VERSION, "name": "x" });
        tokio::fs::write(&path, serde_json::to_vec_pretty(&data).unwrap())
            .await
            .unwrap();
        migrate_if_needed(&path).await.unwrap();
        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["version"], CURRENT_VERSION);
    }

    #[tokio::test]
    async fn test_migrate_future_version_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        let data = serde_json::json!({"version": 999, "name": "x" });
        tokio::fs::write(&path, serde_json::to_vec_pretty(&data).unwrap())
            .await
            .unwrap();
        assert!(migrate_if_needed(&path).await.is_err());
    }
}
