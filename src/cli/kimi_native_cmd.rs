use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
pub(crate) struct KimiNativeArgs {
    #[command(subcommand)]
    pub command: KimiNativeCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum KimiNativeCommands {
    /// Sync OMK assets for current Kimi surfaces (project + user scope)
    Sync {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(short, long, help = "Force overwrite even if files exist")]
        force: bool,
        #[arg(long, help = "Show what would be done without making changes")]
        dry_run: bool,
    },
    /// Validate Kimi-native configuration and assets
    Doctor {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Output results as JSON")]
        json: bool,
    },
    /// Install OMK role assets into the current project's Kimi workspace
    Install {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Show what would be installed without making changes")]
        dry_run: bool,
    },
    /// List bundled OMK role agent templates
    Agents,
    /// List bundled OMK project hook templates
    Hooks,
    /// List discovered OMK skills in the local data directory
    Skills,
    /// Rollback OMK-installed Kimi assets from .kimi/
    Rollback {
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Show what would be removed without making changes")]
        dry_run: bool,
    },
}

pub(crate) async fn run(args: KimiNativeArgs) -> Result<()> {
    match args.command {
        KimiNativeCommands::Sync {
            dir,
            force,
            dry_run,
        } => cmd_sync(&dir, force, dry_run).await,
        KimiNativeCommands::Doctor { dir, json } => cmd_doctor(&dir, json).await,
        KimiNativeCommands::Install { dir, dry_run } => cmd_install(&dir, dry_run).await,
        KimiNativeCommands::Agents => cmd_agents(),
        KimiNativeCommands::Hooks => cmd_hooks(),
        KimiNativeCommands::Skills => cmd_skills().await,
        KimiNativeCommands::Rollback { dir, dry_run } => cmd_rollback(&dir, dry_run).await,
    }
}

async fn cmd_sync(dir: &std::path::Path, force: bool, dry_run: bool) -> Result<()> {
    info!(dir = %dir.display(), force, dry_run, "Syncing Kimi-native assets");

    let report = crate::kimi_native::sync::sync_project_assets(dir, force, dry_run).await?;

    if dry_run {
        println!("🔍 Dry run — no changes made");
        if !report.would_update.is_empty() {
            println!("\n  Would update:");
            for item in &report.would_update {
                println!("    ~ {}", item);
            }
        }
        if !report.would_create.is_empty() {
            println!("\n  Would create:");
            for item in &report.would_create {
                println!("    + {}", item);
            }
        }
    } else {
        println!("✅ Sync complete");
        if !report.created.is_empty() {
            println!("\n  Created:");
            for item in &report.created {
                println!("    + {}", item);
            }
        }
        if !report.updated.is_empty() {
            println!("\n  Updated:");
            for item in &report.updated {
                println!("    ~ {}", item);
            }
        }
    }
    if !report.unchanged.is_empty() {
        println!("\n  Unchanged:");
        for item in &report.unchanged {
            println!("    = {}", item);
        }
    }
    if !report.errors.is_empty() {
        println!("\n  Errors:");
        for err in &report.errors {
            println!("    ✗ {}", err);
        }
    }

    // Also sync user-level
    let user_report = crate::kimi_native::sync::sync_user_assets(force, dry_run).await?;
    if dry_run {
        if !user_report.would_update.is_empty() {
            println!("\n  User-level would update:");
            for item in &user_report.would_update {
                println!("    ~ {}", item);
            }
        }
        if !user_report.would_create.is_empty() {
            println!("\n  User-level would create:");
            for item in &user_report.would_create {
                println!("    + {}", item);
            }
        }
    } else {
        if !user_report.created.is_empty() {
            println!("\n  User-level created:");
            for item in &user_report.created {
                println!("    + {}", item);
            }
        }
        if !user_report.updated.is_empty() {
            println!("\n  User-level updated:");
            for item in &user_report.updated {
                println!("    ~ {}", item);
            }
        }
    }

    // Summary line
    if !dry_run {
        let proj_written = report.files_written();
        let proj_unchanged = report.files_unchanged();
        let user_written = user_report.files_written();
        println!(
            "\nProject: {} files synced ({} created, {} updated), {} unchanged, {} backups. User: {} files synced.",
            proj_written,
            report.created.len(),
            report.updated.len(),
            proj_unchanged,
            report.backups_created.len(),
            user_written,
        );
    }

    Ok(())
}

async fn cmd_doctor(dir: &std::path::Path, json_output: bool) -> Result<()> {
    use crate::kimi_native::diagnostics;

    let results = diagnostics::diagnose_project(dir).await?;

    if json_output {
        let json = serde_json::to_string_pretty(&results)?;
        println!("{}", json);
        return Ok(());
    }

    let mut issues = 0;

    println!("🩺 Kimi-native doctor — {}", dir.display());
    println!("{}", "=".repeat(50));

    for r in &results {
        match r.severity {
            diagnostics::Severity::Ok => println!("  ✅ {}", r.message),
            diagnostics::Severity::Warning => {
                println!("  ⚠️  {}", r.message);
                issues += 1;
            }
            diagnostics::Severity::Error => {
                println!("  ❌ {}", r.message);
                issues += 1;
            }
        }
        if let Some(ref fix) = r.fix_hint {
            println!("     → {}", fix);
        }
    }

    println!("\n{}", "=".repeat(50));
    if issues == 0 {
        println!("🎉 All checks passed!");
    } else {
        println!(
            "⚠️  {} issue(s) found. Follow the repair commands above.",
            issues
        );
    }

    Ok(())
}

async fn cmd_install(dir: &std::path::Path, dry_run: bool) -> Result<()> {
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

fn cmd_agents() -> Result<()> {
    let agents = crate::kimi_native::agent_spec::default_role_agents();
    println!("📋 OMK Role Agents ({}):", agents.len());
    for agent in &agents {
        println!(
            "  • {} — {}",
            agent.id,
            agent.system_prompt.split('.').next().unwrap_or("")
        );
    }
    Ok(())
}

fn cmd_hooks() -> Result<()> {
    let hooks = crate::kimi_native::hook_spec::default_project_hooks();
    println!("📋 OMK Project Hooks ({}):", hooks.hooks.len());
    for hook in &hooks.hooks {
        println!("  • {:?}", hook.event);
    }
    println!("\n  Scripts ({}):", hooks.scripts.len());
    for (name, _) in &hooks.scripts {
        println!("    • {}", name);
    }
    Ok(())
}

async fn cmd_skills() -> Result<()> {
    let data_dir = crate::runtime::config::data_dir();
    let skills_dir = data_dir.join("skills");

    if !skills_dir.exists() {
        println!("ℹ️  No skills directory found at {}", skills_dir.display());
        println!("   Skills are discovered from .kimi/skills/, .claude/skills/, etc.");
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&skills_dir).await?;
    let mut skills = vec![];
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            skills.push(name);
        }
    }

    println!("📋 Available Skills ({}):", skills.len());
    for skill in &skills {
        println!("  • {}", skill);
    }
    Ok(())
}

async fn cmd_rollback(dir: &std::path::Path, dry_run: bool) -> Result<()> {
    use crate::kimi_native::rollback;

    let report = rollback::rollback(dir, dry_run).await?;
    if report.manifest_missing {
        println!(
            "ℹ️  No OMK manifest found at {}. Nothing to rollback.",
            dir.display()
        );
        return Ok(());
    }

    if dry_run {
        println!("🔍 Dry run — no changes made");
    } else {
        println!("🔄 Rolling back OMK Kimi assets...");
    }

    if !report.restored.is_empty() {
        if dry_run {
            println!("\n  Would restore:");
        } else {
            println!("\n  Restored from backup:");
        }
        for f in &report.restored {
            println!("    ← {}", f.display());
        }
    }

    if !report.removed.is_empty() {
        if dry_run {
            println!("\n  Would remove:");
        } else {
            println!("\n  Removed:");
        }
        for f in &report.removed {
            println!("    - {}", f.display());
        }
    }

    if !report.skipped.is_empty() {
        println!("\n  Skipped:");
        for f in &report.skipped {
            println!("    = {}", f.display());
        }
    }

    if !report.errors.is_empty() {
        println!("\n  Errors:");
        for e in &report.errors {
            println!("    ✗ {}", e);
        }
    }

    println!(
        "\n  Report: restored={}, removed={}, skipped={}, errors={}",
        report.restored.len(),
        report.removed.len(),
        report.skipped.len(),
        report.errors.len()
    );

    if dry_run {
        println!("\n🔍 Dry run complete — no changes made.");
    } else if report.errors.is_empty() {
        println!("\n✅ Rollback complete.");
    } else {
        println!("\n⚠️  Rollback completed with errors.");
        return Err(anyhow::anyhow!(
            "Rollback failed for {} file(s)",
            report.errors.len()
        ));
    }

    Ok(())
}
