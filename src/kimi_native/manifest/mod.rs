mod checksum;
mod ops;
mod path;
mod types;

#[cfg(test)]
mod tests;

pub use checksum::{compute_checksum, compute_checksum_bytes, is_identical, maybe_backup};
pub use types::{
    AssetManifest, BackupEntry, EntryKind, ManifestEntry, RollbackReport, MANIFEST_SCHEMA_VERSION,
};
