use anyhow::Result;
use tracing::info;

pub(super) async fn cmd_sync(dir: &std::path::Path, force: bool, dry_run: bool) -> Result<()> {
    info!(dir = %dir.display(), force, dry_run, "Syncing Kimi-native assets");

    let report = crate::kimi_native::sync::sync_project_assets(dir, force, dry_run).await?;

    if dry_run {
        println!("🔍 Dry run — no changes made");
        if !report.would_update.is_empty() {
            println!("\n  {} would update:", report.scope.as_label());
            for item in &report.would_update {
                println!("    ~ {}", item);
            }
        }
        if !report.would_create.is_empty() {
            println!("\n  {} would create:", report.scope.as_label());
            for item in &report.would_create {
                println!("    + {}", item);
            }
        }
    } else {
        println!("✅ Sync complete");
        if !report.created.is_empty() {
            println!("\n  {} created:", report.scope.as_label());
            for item in &report.created {
                println!("    + {}", item);
            }
        }
        if !report.updated.is_empty() {
            println!("\n  {} updated:", report.scope.as_label());
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
            println!("\n  {} would update:", user_report.scope.as_label());
            for item in &user_report.would_update {
                println!("    ~ {}", item);
            }
        }
        if !user_report.would_create.is_empty() {
            println!("\n  {} would create:", user_report.scope.as_label());
            for item in &user_report.would_create {
                println!("    + {}", item);
            }
        }
    } else {
        if !user_report.created.is_empty() {
            println!("\n  {} created:", user_report.scope.as_label());
            for item in &user_report.created {
                println!("    + {}", item);
            }
        }
        if !user_report.updated.is_empty() {
            println!("\n  {} updated:", user_report.scope.as_label());
            for item in &user_report.updated {
                println!("    ~ {}", item);
            }
        }
    }

    if dry_run {
        println!(
            "\n{} summary: {} planned ({} create, {} update), {} unchanged.",
            report.scope.as_label(),
            report.files_planned(),
            report.would_create.len(),
            report.would_update.len(),
            report.files_unchanged(),
        );
        println!(
            "{} summary: {} planned ({} create, {} update), {} unchanged.",
            user_report.scope.as_label(),
            user_report.files_planned(),
            user_report.would_create.len(),
            user_report.would_update.len(),
            user_report.files_unchanged(),
        );
    } else {
        let proj_written = report.files_written();
        let proj_unchanged = report.files_unchanged();
        let user_written = user_report.files_written();
        let user_unchanged = user_report.files_unchanged();
        println!(
            "\n{} summary: {} synced ({} created, {} updated), {} unchanged, {} backups.",
            report.scope.as_label(),
            proj_written,
            report.created.len(),
            report.updated.len(),
            proj_unchanged,
            report.backups_created.len(),
        );
        println!(
            "{} summary: {} synced ({} created, {} updated), {} unchanged, {} backups.",
            user_report.scope.as_label(),
            user_written,
            user_report.created.len(),
            user_report.updated.len(),
            user_unchanged,
            user_report.backups_created.len(),
        );
    }

    Ok(())
}
