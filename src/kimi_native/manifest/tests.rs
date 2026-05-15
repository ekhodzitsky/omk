use super::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn test_compute_checksum() {
    let checksum = compute_checksum("hello");
    assert_eq!(checksum.len(), 16);
    // FNV-1a 64-bit hash of "hello"
    assert_eq!(checksum, "a430d84680aabd0b");
}

#[tokio::test]
async fn test_drifted_files_missing() {
    let dir = TempDir::new().unwrap();
    let mut manifest = AssetManifest::new(dir.path());
    manifest
        .add_file(Path::new("test.txt"), EntryKind::Other)
        .await;
    let drifted = manifest.drifted_files(dir.path()).await;
    assert_eq!(drifted.len(), 1);
    assert_eq!(drifted[0].0, PathBuf::from("test.txt"));
    assert!(drifted[0].1.is_none());
}

#[tokio::test]
async fn test_drifted_files_checksum_mismatch() {
    let dir = TempDir::new().unwrap();
    let mut manifest = AssetManifest::new(dir.path());
    tokio::fs::write(dir.path().join("test.txt"), "original")
        .await
        .unwrap();
    manifest
        .add_file(Path::new("test.txt"), EntryKind::Other)
        .await;

    tokio::fs::write(dir.path().join("test.txt"), "modified")
        .await
        .unwrap();
    let drifted = manifest.drifted_files(dir.path()).await;
    assert_eq!(drifted.len(), 1);
    assert_eq!(drifted[0].0, PathBuf::from("test.txt"));
    assert!(drifted[0].1.is_some());
}

#[tokio::test]
async fn test_drifted_files_unchanged() {
    let dir = TempDir::new().unwrap();
    let mut manifest = AssetManifest::new(dir.path());
    tokio::fs::write(dir.path().join("test.txt"), "same")
        .await
        .unwrap();
    manifest
        .add_file(Path::new("test.txt"), EntryKind::Other)
        .await;
    let drifted = manifest.drifted_files(dir.path()).await;
    assert!(drifted.is_empty());
}

#[tokio::test]
async fn test_verify_checksum_missing() {
    let dir = TempDir::new().unwrap();
    let mut manifest = AssetManifest::new(dir.path());
    tokio::fs::write(dir.path().join("test.txt"), "content")
        .await
        .unwrap();
    manifest
        .add_file(Path::new("test.txt"), EntryKind::Other)
        .await;
    tokio::fs::remove_file(dir.path().join("test.txt"))
        .await
        .unwrap();

    let verified = manifest.verify_checksum(dir.path()).await;
    assert_eq!(verified.len(), 1);
    assert_eq!(verified[0].0, PathBuf::from("test.txt"));
    assert!(verified[0].1.is_some()); // expected
    assert!(verified[0].2.is_none()); // actual
}

#[tokio::test]
async fn test_verify_checksum_mismatch() {
    let dir = TempDir::new().unwrap();
    let mut manifest = AssetManifest::new(dir.path());
    tokio::fs::write(dir.path().join("test.txt"), "original")
        .await
        .unwrap();
    manifest
        .add_file(Path::new("test.txt"), EntryKind::Other)
        .await;

    tokio::fs::write(dir.path().join("test.txt"), "modified")
        .await
        .unwrap();
    let verified = manifest.verify_checksum(dir.path()).await;
    assert_eq!(verified.len(), 1);
    assert_eq!(verified[0].0, PathBuf::from("test.txt"));
    assert!(verified[0].1.is_some()); // expected
    assert!(verified[0].2.is_some()); // actual
    assert_ne!(verified[0].1, verified[0].2);
}

#[tokio::test]
async fn test_manifest_schema_version_too_new() {
    let dir = TempDir::new().unwrap();
    tokio::fs::create_dir_all(dir.path().join(".kimi"))
        .await
        .unwrap();
    let manifest = AssetManifest::new(dir.path());
    manifest.save(dir.path()).await.unwrap();

    // Modify the manifest on disk to have a future version
    let path = AssetManifest::manifest_path(dir.path());
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    let content = content.replace("\"version\": 1", "\"version\": 999");
    tokio::fs::write(&path, content).await.unwrap();

    let result = AssetManifest::load(dir.path()).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("newer than supported"));
}

#[tokio::test]
async fn test_manifest_schema_version_outdated() {
    let dir = TempDir::new().unwrap();
    tokio::fs::create_dir_all(dir.path().join(".kimi"))
        .await
        .unwrap();
    let manifest = AssetManifest::new(dir.path());
    manifest.save(dir.path()).await.unwrap();

    let path = AssetManifest::manifest_path(dir.path());
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    let content = content.replace("\"version\": 1", "\"version\": 0");
    tokio::fs::write(&path, content).await.unwrap();

    let result = AssetManifest::load(dir.path()).await;
    assert!(result.is_ok());
    let loaded = result.unwrap();
    assert!(loaded.is_some());
}

#[tokio::test]
async fn test_manifest_rejects_parent_traversal_paths() {
    let dir = TempDir::new().unwrap();
    tokio::fs::create_dir_all(dir.path().join(".kimi"))
        .await
        .unwrap();
    let manifest_path = AssetManifest::manifest_path(dir.path());

    let payload = serde_json::json!({
        "version": MANIFEST_SCHEMA_VERSION,
        "created_at": chrono::Utc::now(),
        "omk_version": env!("CARGO_PKG_VERSION"),
        "project_dir": dir.path(),
        "files": [{
            "path": "../../etc/passwd",
            "kind": "other",
            "checksum": null
        }],
        "directories": []
    });
    tokio::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&payload).unwrap(),
    )
    .await
    .unwrap();

    let result = AssetManifest::load(dir.path()).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("escapes allowed roots"));
}

#[tokio::test]
async fn test_manifest_rejects_absolute_paths() {
    let dir = TempDir::new().unwrap();
    tokio::fs::create_dir_all(dir.path().join(".kimi"))
        .await
        .unwrap();
    let manifest_path = AssetManifest::manifest_path(dir.path());

    let payload = serde_json::json!({
        "version": MANIFEST_SCHEMA_VERSION,
        "created_at": chrono::Utc::now(),
        "omk_version": env!("CARGO_PKG_VERSION"),
        "project_dir": dir.path(),
        "files": [{
            "path": dir.path().join(".kimi/agents/architect/agent.yaml"),
            "kind": "agent_spec",
            "checksum": null
        }],
        "directories": []
    });
    tokio::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&payload).unwrap(),
    )
    .await
    .unwrap();

    let result = AssetManifest::load(dir.path()).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must be relative to the project root"));
}
