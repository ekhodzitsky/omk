use anyhow::Result;
use tracing::info;

pub(super) async fn cmd_install(dir: &std::path::Path, dry_run: bool) -> Result<()> {
    info!(dir = %dir.display(), dry_run, "Installing Kimi-native assets");

    let report = crate::kimi_native::installer::install_project_assets(dir, dry_run).await?;

    if dry_run {
        println!("🔍 Dry run — no changes made");
        if !report.would_install.is_empty() {
            println!("\n  Would install:");
            for item in &report.would_install {
                println!("    + {}", item);
            }
        }
    } else {
        println!("✅ Installation complete");
        if !report.agents_installed.is_empty() {
            println!("\n  Agents installed:");
            for a in &report.agents_installed {
                println!("    + {}", a);
            }
        }
        if !report.hooks_installed.is_empty() {
            println!("\n  Hooks installed:");
            for h in &report.hooks_installed {
                println!("    + {}", h);
            }
        }
        if report.skills_linked {
            println!("\n  Skills: linked");
        }
    }
    if !report.errors.is_empty() {
        println!("\n  Errors:");
        for err in &report.errors {
            println!("    ✗ {}", err);
        }
    }

    Ok(())
}
