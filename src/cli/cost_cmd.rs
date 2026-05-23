use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: CostCommands,
}

#[derive(Subcommand, Debug)]
pub enum CostCommands {
    /// Show cost report
    Report,
    /// Reset cost tracking data
    Reset,
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        CostCommands::Report => show_report().await,
        CostCommands::Reset => reset_costs().await,
    }
}

async fn show_report() -> Result<()> {
    let sink = crate::cost::file_sink::JsonFileCostSink::new(
        crate::runtime::config::state_dir().join("costs.json"),
    );
    let tracker = crate::cost::tracker::CostTracker::new(sink);
    let report = tracker.report().await?;
    println!("{}", report);
    Ok(())
}

async fn reset_costs() -> Result<()> {
    let sink = crate::cost::file_sink::JsonFileCostSink::new(
        crate::runtime::config::state_dir().join("costs.json"),
    );
    let tracker = crate::cost::tracker::CostTracker::new(sink);
    tracker.clear().await?;
    println!("✓ Cost tracking data reset");
    Ok(())
}
