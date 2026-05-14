//! `omk goal budget` / `omk goal budget-add` rendering.
//!
//! Read-side budget reporting and budget extension acknowledgement. Kept in
//! its own file because the formatter helpers (`print_budget_summary_*`,
//! `format_optional_secs`) are non-trivial and reused for both `text` and
//! `md` output.

use anyhow::Result;

use super::super::OutputFormat;

pub(in crate::cli::goal) async fn cmd_budget(goal_id: &str, format: OutputFormat) -> Result<()> {
    let report = crate::runtime::goal::goal_budget(goal_id).await?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Md => {
            println!("# Goal Budget {}", report.goal_id);
            println!();
            print_budget_summary_md(&report);
            println!();
            println!("## Checkpoints");
            for checkpoint in &report.checkpoints {
                println!(
                    "- `{}` `{}` status=`{}` phase=`{}` remaining={}",
                    checkpoint.recorded_at,
                    checkpoint.label,
                    checkpoint.status,
                    checkpoint.phase,
                    format_optional_secs(checkpoint.remaining_budget_secs)
                );
            }
        }
        OutputFormat::Text => {
            println!("Goal budget {}", report.goal_id);
            print_budget_summary_text(&report);
            println!("Checkpoints: {}", report.checkpoints.len());
            for checkpoint in &report.checkpoints {
                println!(
                    "  {}  {:18} status={} phase={} remaining={}",
                    checkpoint.recorded_at,
                    checkpoint.label,
                    checkpoint.status,
                    checkpoint.phase,
                    format_optional_secs(checkpoint.remaining_budget_secs)
                );
            }
        }
    }

    Ok(())
}

pub(in crate::cli::goal) async fn cmd_budget_add(
    goal_id: &str,
    time: Option<String>,
    tokens: Option<u64>,
    usd: Option<f64>,
) -> Result<()> {
    let goal = crate::runtime::goal::add_goal_budget_limits(
        goal_id,
        crate::runtime::goal::GoalBudgetAdd {
            time: time.clone(),
            tokens,
            usd,
        },
    )
    .await?;
    println!("Budget added: {}", goal.goal_id);
    println!("Status: {}", goal.status);
    println!(
        "Budget time: {}",
        goal.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!(
        "Budget tokens: {}",
        goal.budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string())
    );
    println!(
        "Budget USD: {}",
        goal.budget_usd
            .map(|value| format!("{value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string())
    );
    if let Some(time) = time {
        println!("Time added: {time}");
    }
    if let Some(tokens) = tokens {
        println!("Tokens added: {tokens}");
    }
    if let Some(usd) = usd {
        println!("USD added: {usd:.6}");
    }
    Ok(())
}

fn print_budget_summary_text(report: &crate::runtime::goal::GoalBudgetReport) {
    println!(
        "Budget time: {}",
        report.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!("Total: {}", format_optional_secs(report.total_budget_secs));
    println!(
        "Tokens: used={} budget={} remaining={}",
        report.used_tokens,
        report
            .budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string())
    );
    println!(
        "Cost: estimated=${:.6} budget={} remaining={}",
        report.estimated_cost_usd,
        report
            .budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string())
    );
    if let Some(latest) = &report.latest {
        println!(
            "Elapsed: {}",
            format_optional_secs(Some(latest.elapsed_since_created_secs))
        );
        println!(
            "Remaining: {}",
            format_optional_secs(latest.remaining_budget_secs)
        );
    } else {
        println!("Elapsed: unknown");
        println!("Remaining: unknown");
    }
}

fn print_budget_summary_md(report: &crate::runtime::goal::GoalBudgetReport) {
    println!(
        "- Budget time: `{}`",
        report.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!(
        "- Total: `{}`",
        format_optional_secs(report.total_budget_secs)
    );
    println!(
        "- Tokens: used=`{}` budget=`{}` remaining=`{}`",
        report.used_tokens,
        report
            .budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string())
    );
    println!(
        "- Cost: estimated=`${:.6}` budget=`{}` remaining=`{}`",
        report.estimated_cost_usd,
        report
            .budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string())
    );
    if let Some(latest) = &report.latest {
        println!(
            "- Elapsed: `{}`",
            format_optional_secs(Some(latest.elapsed_since_created_secs))
        );
        println!(
            "- Remaining: `{}`",
            format_optional_secs(latest.remaining_budget_secs)
        );
    } else {
        println!("- Elapsed: `unknown`");
        println!("- Remaining: `unknown`");
    }
}

fn format_optional_secs(value: Option<u64>) -> String {
    value
        .map(|value| format!("{value}s"))
        .unwrap_or_else(|| "unbounded".to_string())
}
