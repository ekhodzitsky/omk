use anyhow::Result;
use std::path::Path;

pub mod types;

mod agents;
mod assets;
mod cli;
mod hooks;
mod skills;

pub use types::{DiagResult, Severity};

pub async fn diagnose_project(dir: &Path) -> Result<Vec<DiagResult>> {
    let mut results = Vec::new();
    let kimi_dir = dir.join(".kimi");
    let agents_dir = kimi_dir.join("agents");
    let hooks_dir = kimi_dir.join("hooks");

    // Check .kimi/ directory exists
    if kimi_dir.exists() {
        results.push(DiagResult {
            severity: Severity::Ok,
            message: format!(".kimi/ directory found at {}", kimi_dir.display()),
            fix_hint: None,
        });
    } else {
        results.push(DiagResult {
            severity: Severity::Warning,
            message: format!(".kimi/ directory not found at {}", kimi_dir.display()),
            fix_hint: Some("Run `omk kimi install` to create it".to_string()),
        });
    }

    agents::check_agents(&agents_dir, &mut results).await;
    hooks::check_hooks(&hooks_dir, dir, &mut results).await;
    hooks::check_hook_configs(dir, &kimi_dir, &mut results).await;
    skills::check_skills(&kimi_dir, &mut results).await?;
    assets::check_agents_md(dir, &mut results);
    assets::check_manifest(dir, &mut results).await;
    cli::check_kimi_cli(&mut results).await;

    Ok(results)
}
