use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GoalSourceSurfaces {
    pub(crate) commands: Vec<String>,
    pub(crate) api_files: Vec<PathBuf>,
}

pub(crate) async fn detect_source_project_surfaces(
    project_dir: &Path,
) -> Result<GoalSourceSurfaces> {
    let mut surfaces = GoalSourceSurfaces::default();
    detect_rust_surfaces(project_dir, &mut surfaces).await?;
    detect_node_surfaces(project_dir, &mut surfaces).await?;
    detect_python_surfaces(project_dir, &mut surfaces).await?;
    surfaces.commands.sort();
    surfaces.commands.dedup();
    surfaces.api_files.sort();
    surfaces.api_files.dedup();
    Ok(surfaces)
}

async fn detect_rust_surfaces(project_dir: &Path, surfaces: &mut GoalSourceSurfaces) -> Result<()> {
    if !exists(project_dir.join("Cargo.toml")).await? {
        return Ok(());
    }
    surfaces
        .commands
        .push("cargo check --all-targets".to_string());
    surfaces.commands.push("cargo test".to_string());
    push_api_if_exists(project_dir, surfaces, "src/lib.rs").await?;
    push_api_if_exists(project_dir, surfaces, "src/main.rs").await?;
    Ok(())
}

async fn detect_node_surfaces(project_dir: &Path, surfaces: &mut GoalSourceSurfaces) -> Result<()> {
    if !exists(project_dir.join("package.json")).await? {
        return Ok(());
    }
    surfaces.commands.push("npm test".to_string());
    surfaces.commands.push("npm run build".to_string());
    for path in [
        "src/index.ts",
        "src/index.tsx",
        "src/index.js",
        "src/main.ts",
        "src/main.tsx",
        "src/main.js",
    ] {
        push_api_if_exists(project_dir, surfaces, path).await?;
    }
    Ok(())
}

async fn detect_python_surfaces(
    project_dir: &Path,
    surfaces: &mut GoalSourceSurfaces,
) -> Result<()> {
    let has_pyproject = exists(project_dir.join("pyproject.toml")).await?;
    let has_setup = exists(project_dir.join("setup.py")).await?;
    if !has_pyproject && !has_setup {
        return Ok(());
    }
    if exists(project_dir.join("tests")).await? {
        surfaces.commands.push("python -m pytest".to_string());
    }
    let mut entries = tokio::fs::read_dir(project_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let path = entry.path();
        if exists(path.join("__init__.py")).await? {
            let relative = path
                .strip_prefix(project_dir)
                .map(PathBuf::from)
                .unwrap_or_else(|_| path.clone())
                .join("__init__.py");
            surfaces.api_files.push(relative);
        }
    }
    Ok(())
}

async fn push_api_if_exists(
    project_dir: &Path,
    surfaces: &mut GoalSourceSurfaces,
    relative: &str,
) -> Result<()> {
    if exists(project_dir.join(relative)).await? {
        surfaces.api_files.push(PathBuf::from(relative));
    }
    Ok(())
}

async fn exists(path: impl AsRef<Path>) -> Result<bool> {
    tokio::fs::try_exists(path).await.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn detects_rust_command_and_api_surfaces() {
        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n",
        )
        .expect("Cargo.toml");
        fs::create_dir_all(project.path().join("src")).expect("src dir");
        fs::write(
            project.path().join("src/lib.rs"),
            "pub fn answer() -> u8 { 42 }\n",
        )
        .expect("lib.rs");

        let surfaces = detect_source_project_surfaces(project.path())
            .await
            .expect("surface detection");

        assert!(surfaces
            .commands
            .iter()
            .any(|command| command == "cargo test"));
        assert!(surfaces
            .commands
            .iter()
            .any(|command| command == "cargo check --all-targets"));
        assert!(surfaces
            .api_files
            .iter()
            .any(|path| path == &PathBuf::from("src/lib.rs")));
    }

    #[tokio::test]
    async fn detects_python_command_and_api_surfaces() {
        let project = tempfile::tempdir().expect("project");
        fs::write(
            project.path().join("pyproject.toml"),
            "[project]\nname='demo'\n",
        )
        .expect("pyproject.toml");
        fs::create_dir_all(project.path().join("demo")).expect("package dir");
        fs::write(
            project.path().join("demo/__init__.py"),
            "def answer(): return 42\n",
        )
        .expect("__init__.py");
        fs::create_dir_all(project.path().join("tests")).expect("tests dir");

        let surfaces = detect_source_project_surfaces(project.path())
            .await
            .expect("surface detection");

        assert!(surfaces
            .commands
            .iter()
            .any(|command| command == "python -m pytest"));
        assert!(surfaces
            .api_files
            .iter()
            .any(|path| path == &PathBuf::from("demo/__init__.py")));
    }
}
