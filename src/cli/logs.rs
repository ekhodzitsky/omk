use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Number of lines to show (like tail -n)
    #[arg(short, long, default_value = "50")]
    lines: usize,

    /// Follow log output (like tail -f)
    #[arg(short, long)]
    follow: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let log_dir = crate::runtime::config::state_dir().join("logs");
    let log_file = log_dir.join("omk.log");

    if !log_file.exists() {
        println!("No log file found at {}", log_file.display());
        return Ok(());
    }

    if args.follow {
        println!("Following {} (Ctrl+C to stop)...", log_file.display());
        let status = std::process::Command::new("tail")
            .args(["-f", "-n", &args.lines.to_string(), log_file.to_str().unwrap()])
            .status()
            .context("Failed to run tail command")?;

        if !status.success() {
            anyhow::bail!("tail command failed");
        }
    } else {
        let output = std::process::Command::new("tail")
            .args(["-n", &args.lines.to_string(), log_file.to_str().unwrap()])
            .output()
            .context("Failed to run tail command")?;

        if !output.status.success() {
            anyhow::bail!("tail command failed");
        }

        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    Ok(())
}
