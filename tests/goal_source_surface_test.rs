use std::fs;
use std::path::PathBuf;

mod runtime_goal {
    pub(crate) mod state {
        pub(crate) fn normalize_goal(goal: &str) -> String {
            goal.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }

    pub(crate) mod oracle {
        #![allow(dead_code)]
        include!("../src/runtime/goal/oracle.rs");
    }
}

use runtime_goal::oracle::surface::detect_source_project_surfaces;

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
