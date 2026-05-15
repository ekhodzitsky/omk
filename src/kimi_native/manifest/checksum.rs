use std::path::Path;

pub fn compute_checksum(content: &str) -> String {
    format!("{:016x}", fnv1a_64(content.as_bytes()))
}

pub fn compute_checksum_bytes(data: &[u8]) -> String {
    format!("{:016x}", fnv1a_64(data))
}

/// Check whether `path` exists and its content is byte-identical or
/// checksum-identical to `new_content`.
pub async fn is_identical(path: &Path, new_content: &str) -> bool {
    if !path.exists() {
        return false;
    }
    match tokio::fs::read(path).await {
        Ok(existing) => {
            let new = new_content.as_bytes();
            existing == new || compute_checksum_bytes(&existing) == compute_checksum_bytes(new)
        }
        Err(_) => false,
    }
}

/// Create a backup of `path` if it exists and its content differs from `new_content`.
/// Returns the backup path on success, or None if no backup was needed or creation failed.
pub async fn maybe_backup(path: &Path, new_content: &str) -> Option<String> {
    if !path.exists() {
        return None;
    }
    if is_identical(path, new_content).await {
        return None;
    }
    let timestamp = chrono::Utc::now().timestamp();
    let backup_path = format!("{}.omk-backup-{}", path.display(), timestamp);
    match tokio::fs::copy(path, &backup_path).await {
        Ok(_) => Some(backup_path),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to create backup");
            None
        }
    }
}

fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
