use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Autonomous execution with single lead agent
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Task description
    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub task: Vec<String>,

    /// Working directory
    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    /// Enable Ralph persistence loop
    #[arg(long)]
    pub ralph: bool,

    /// Autopilot run name (auto-generated if omitted)
    #[arg(short, long)]
    pub name: Option<String>,
}

pub async fn run(args: Args) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let name = args.name.unwrap_or_else(|| {
        format!(
            "ap-{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        )
    });

    crate::runtime::autopilot::run_autopilot(&name, &task, &args.dir, args.ralph).await
}
