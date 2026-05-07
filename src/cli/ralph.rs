use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

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
}

pub async fn run(args: Args) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    crate::runtime::ralph::run_ralph(&task, &args.dir, args.max_iterations).await
}
