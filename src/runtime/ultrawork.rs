use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

/// A single completed ultrawork job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UltraworkJob {
    pub id: usize,
    pub prompt: String,
    pub result: Option<String>,
    pub duration_ms: u64,
    pub success: bool,
}

/// Run multiple kimi prompts in parallel with a concurrency limit.
pub async fn run_ultrawork(
    tasks: Vec<String>,
    dir: &Path,
    concurrency: usize,
    output: Option<PathBuf>,
) -> Result<Vec<UltraworkJob>> {
    if tasks.is_empty() {
        anyhow::bail!("No tasks provided. Pass tasks as arguments or use --file.");
    }

    let started_at = chrono::Utc::now();
    let concurrency = concurrency.max(1);

    // Show rough cost estimate
    let rough_estimate = crate::cost::estimator::estimate_cost(
        60 * tasks.len() as u64,
        tasks.len(),
        1,
        crate::cost::estimator::PricingTier::Standard,
    );
    println!("  Estimated cost: {}", rough_estimate.formatted());

    // Load AGENTS.md if present
    let agents_md = crate::agents::load_project_agents(dir).await.ok().flatten();

    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut join_set = tokio::task::JoinSet::new();

    println!();
    println!(
        "⚡ Ultrawork: {} jobs, concurrency {}",
        tasks.len(),
        concurrency
    );
    println!();

    for (i, task) in tasks.iter().enumerate() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| anyhow::anyhow!("Semaphore error: {e}"))?;

        let prompt = if let Some(ref manifest) = agents_md {
            format!(
                "{}\n\n{}",
                task,
                crate::agents::inject_agents_context(manifest, task, "worker")
            )
        } else {
            task.clone()
        };

        let dir = dir.to_path_buf();
        let task = task.clone();
        join_set.spawn(async move {
            let start = std::time::Instant::now();
            let result = run_kimi(&prompt, &dir).await;
            let duration_ms = start.elapsed().as_millis() as u64;
            let success = result.is_ok();
            drop(permit);
            UltraworkJob {
                id: i,
                prompt: task,
                result: result.ok(),
                duration_ms,
                success,
            }
        });
    }

    let mut jobs = Vec::with_capacity(tasks.len());
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(job) => {
                let icon = if job.success { "✓" } else { "✗" };
                println!(
                    "  {} Job {:2} completed in {:>5}ms  {}",
                    icon,
                    job.id + 1,
                    job.duration_ms,
                    job.prompt.chars().take(50).collect::<String>()
                );
                jobs.push(job);
            }
            Err(e) => {
                warn!(error = %e, "Ultrawork job panicked");
            }
        }
    }

    // Summary
    let success_count = jobs.iter().filter(|j| j.success).count();
    let total_duration_ms: u64 = jobs.iter().map(|j| j.duration_ms).sum();
    let total_duration_secs = total_duration_ms / 1000;

    println!();
    println!(
        "⚡ Ultrawork complete: {}/{} jobs succeeded in {}s",
        success_count,
        jobs.len(),
        total_duration_secs
    );

    // Save output if requested
    if let Some(path) = output {
        let json = serde_json::to_string_pretty(&jobs)?;
        tokio::fs::write(&path, json).await?;
        println!("  Results saved to {}", path.display());
    }

    // Record cost
    let cost = crate::cost::estimator::estimate_cost(
        total_duration_secs,
        jobs.len(),
        1,
        crate::cost::estimator::PricingTier::Standard,
    );
    let _ = crate::runtime::session::record_session_end(
        "ultrawork",
        &format!("ultrawork-{}-jobs", jobs.len()),
        started_at,
        cost,
        crate::notifications::NotificationEvent::UltraworkComplete {
            jobs_total: jobs.len(),
            jobs_success: success_count,
            duration_secs: total_duration_secs,
        },
    )
    .await;

    info!(
        jobs_total = jobs.len(),
        jobs_success = success_count,
        duration_secs = total_duration_secs,
        "Ultrawork session complete"
    );

    Ok(jobs)
}

async fn run_kimi(prompt: &str, dir: &Path) -> Result<String> {
    let output = Command::new("kimi")
        .args(["-p", prompt])
        .current_dir(dir)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        anyhow::bail!("kimi exited with error: {stderr}");
    }

    Ok(format!("{stdout}{stderr}"))
}
