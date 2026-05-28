use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

/// Persistent mode with verify/fix loops
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Task description
    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub task: Vec<String>,

    /// Working directory
    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    /// Max Ralph iterations
    #[arg(short, long, default_value = "10")]
    pub max_iterations: usize,

    /// Resume existing Ralph run for this task
    #[arg(long)]
    pub resume: bool,

    /// Skip confirmation prompts (YOLO mode)
    #[arg(long)]
    pub yolo: bool,
}

pub(crate) async fn run(args: Args, _cancel: CancellationToken) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let summary = crate::runtime::ralph::run_ralph(
        &task,
        &args.dir,
        args.max_iterations,
        args.resume,
        args.yolo,
    )
    .await?;

    let cost = crate::cost::estimator::estimate_ralph_cost(
        summary.duration_secs,
        summary.iterations.unwrap_or(0),
        summary.total_stories.unwrap_or(0),
    );
    let notification = crate::notifications::NotificationEvent::RalphComplete {
        name: summary.name.clone(),
        duration_secs: summary.duration_secs,
        iterations: summary.iterations.unwrap_or(0),
        verified: summary.verified.unwrap_or(0),
        total: summary.total_stories.unwrap_or(0),
    };
    if let Err(e) = crate::cli::session::record_session_end(&summary, cost, notification).await {
        tracing::warn!(error = %e, "Failed to record ralph session end");
    }

    Ok(())
}
