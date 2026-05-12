use anyhow::Result;

pub(super) async fn cmd_doctor(dir: &std::path::Path, json_output: bool) -> Result<()> {
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
