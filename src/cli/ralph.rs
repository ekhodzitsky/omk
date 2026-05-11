use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

/// Persistent mode with verify/fix loops
#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
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

    crate::runtime::ralph::run_ralph(
        &task,
        &args.dir,
        args.max_iterations,
        args.resume,
        args.yolo,
    )
    .await
}
