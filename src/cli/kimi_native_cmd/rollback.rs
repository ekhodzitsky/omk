use anyhow::Result;

pub(super) async fn cmd_rollback(dir: &std::path::Path, dry_run: bool) -> Result<()> {
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
