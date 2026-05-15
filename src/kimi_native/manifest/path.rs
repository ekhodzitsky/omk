use anyhow::{anyhow, Result};
use std::path::{Component, Path, PathBuf};

use super::types::AssetManifest;

pub(crate) fn absolute_root(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn to_project_relative(path: &Path, project_root: &Path) -> Option<PathBuf> {
    let absolute_root = absolute_root(project_root);
    if path.is_absolute() {
        return path.strip_prefix(&absolute_root).ok().map(PathBuf::from);
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    Some(normalized)
}

pub(crate) fn has_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
}

pub(crate) fn has_normal_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::Normal(_)))
}

pub(crate) fn validate_manifest_entry_path(
    path: &Path,
    label: &str,
    project_root: &Path,
) -> Result<()> {
    if path.as_os_str().is_empty() || !has_normal_component(path) {
        return Err(anyhow!(
            "Invalid manifest {} path: '{}'",
            label,
            path.display()
        ));
    }
    if has_parent_traversal(path) {
        return Err(anyhow!(
            "Manifest {} path escapes allowed roots: '{}'",
            label,
            path.display()
        ));
    }

    if path.is_absolute() {
        return Err(anyhow!(
            "Manifest {} path must be relative to the project root: '{}'",
            label,
            path.display()
        ));
    }

    let candidate = project_root.join(path);
    if candidate.starts_with(project_root) {
        return Ok(());
    }

    Err(anyhow!(
        "Manifest {} path resolves outside project root: '{}'",
        label,
        path.display()
    ))
}

pub(crate) fn validate_manifest_paths(
    manifest: &AssetManifest,
    project_dir: &Path,
) -> Result<()> {
    let project_root = absolute_root(project_dir);

    for (index, entry) in manifest.files.iter().enumerate() {
        validate_manifest_entry_path(&entry.path, &format!("files[{}]", index), &project_root)?;
    }

    for (index, dir) in manifest.directories.iter().enumerate() {
        validate_manifest_entry_path(dir, &format!("directories[{}]", index), &project_root)?;
    }

    for (index, backup) in manifest.backups.iter().enumerate() {
        validate_manifest_entry_path(
            &backup.managed_path,
            &format!("backups[{}].managed_path", index),
            &project_root,
        )?;
        validate_manifest_entry_path(
            &backup.backup_path,
            &format!("backups[{}].backup_path", index),
            &project_root,
        )?;
    }

    Ok(())
}
