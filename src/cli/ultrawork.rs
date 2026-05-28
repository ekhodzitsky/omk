use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Parallel burst execution — run multiple tasks concurrently without a team
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Tasks to execute in parallel (one per arg)
    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub tasks: Vec<String>,

    /// Working directory
    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    /// Max concurrent executions
    #[arg(short, long, default_value = "4")]
    pub concurrency: usize,

    /// Read tasks from file (one per line)
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Apply a single prompt to multiple files (glob pattern)
    #[arg(long)]
    pub files: Option<String>,

    /// Unified prompt template for --files mode (use {path} placeholder)
    #[arg(long)]
    pub prompt: Option<String>,

    /// Output results to a JSON file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Skip confirmation prompts
    #[arg(long)]
    pub yolo: bool,
}

pub(crate) async fn run(args: Args) -> Result<()> {
    let mut tasks = args.tasks;

    // Load tasks from file if provided
    if let Some(file) = args.file {
        let content = tokio::fs::read_to_string(&file).await?;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                tasks.push(trimmed.to_string());
            }
        }
    }

    // Expand glob pattern if --files is used
    if let Some(ref pattern) = args.files {
        let template = args
            .prompt
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("--prompt is required when using --files"))?;

        let paths = glob::glob(pattern)?
            .filter_map(Result::ok)
            .filter(|p| p.is_file());

        for path in paths {
            let prompt = template.replace("{path}", &path.to_string_lossy());
            tasks.push(prompt);
        }
    }

    if tasks.is_empty() {
        anyhow::bail!(
            "No tasks provided. Pass tasks as arguments, use --file, or use --files with --prompt."
        );
    }

    if !args.yolo {
        let total = tasks.len();
        println!();
        println!(
            "⚡ Ultrawork: about to run {total} tasks with concurrency {}",
            args.concurrency
        );
        println!("  First 3 tasks:");
        for (i, task) in tasks.iter().take(3).enumerate() {
            println!(
                "    {}. {}",
                i + 1,
                task.chars().take(80).collect::<String>()
            );
        }
        if total > 3 {
            println!("    ... and {} more", total - 3);
        }
        println!();
        println!("Press Enter to continue or Ctrl-C to cancel");
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;
    }

    let (jobs, summary) = crate::runtime::ultrawork::run_ultrawork(
        tasks, &args.dir, args.concurrency, args.output
    ).await?;

    let cost = crate::cost::estimator::estimate_cost(
        summary.duration_secs,
        jobs.len(),
        1,
        crate::cost::estimator::PricingTier::Standard,
    );
    let notification = crate::notifications::NotificationEvent::UltraworkComplete {
        jobs_total: jobs.len(),
        jobs_success: jobs.iter().filter(|j| j.success).count(),
        duration_secs: summary.duration_secs,
    };
    if let Err(e) = crate::cli::session::record_session_end(&summary, cost, notification).await {
        tracing::warn!(error = %e, "Failed to record ultrawork session end");
    }

    Ok(())
}
