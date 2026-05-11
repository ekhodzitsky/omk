use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

/// Autonomous execution with single lead agent
#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
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

    /// Resume an existing autopilot run
    #[arg(long)]
    pub resume: bool,

    /// Skip confirmation prompts (YOLO mode)
    #[arg(long)]
    pub yolo: bool,
}

pub(crate) async fn run(args: Args, _cancel: CancellationToken) -> Result<()> {
    let task = args.task.join(" ");

    if args.resume {
        let name = args
            .name
            .ok_or_else(|| anyhow::anyhow!("--name is required for --resume"))?;
        return crate::runtime::autopilot::resume_autopilot(
            &name, &args.dir, args.ralph, args.yolo,
        )
        .await;
    }

    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let name = args.name.unwrap_or_else(|| {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        format!("ap-{}", &suffix[..8])
    });

    crate::runtime::autopilot::run_autopilot(&name, &task, &args.dir, args.ralph, args.yolo).await
}
