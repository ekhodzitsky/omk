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

pub(crate) async fn run(args: Args) -> Result<()> {
    let log_dir = crate::runtime::config::state_dir().join("logs");
    let log_file = log_dir.join("omk.log");

    if !log_file.exists() {
        println!("No log file found at {}", log_file.display());
        return Ok(());
    }

    if args.follow {
        println!("Following {} (Ctrl+C to stop)...", log_file.display());
        let mut tail_cmd = tokio::process::Command::new("tail");
        tail_cmd
            .args(["-f", "-n", &args.lines.to_string()])
            .arg(&log_file);
        crate::runtime::shell::configure_command(&mut tail_cmd);
        let status = tokio::time::timeout(std::time::Duration::from_secs(300), tail_cmd.status())
            .await
            .context("tail command timed out")?
            .context("Failed to run tail command")?;

        if !status.success() {
            anyhow::bail!("tail command failed");
        }
    } else {
        let mut tail_cmd = tokio::process::Command::new("tail");
        crate::runtime::shell::configure_command(&mut tail_cmd);
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tail_cmd
                .args(["-n", &args.lines.to_string()])
                .arg(&log_file)
                .output(),
        )
        .await
        .context("tail command timed out")?
        .context("Failed to run tail command")?;

        if !output.status.success() {
            anyhow::bail!("tail command failed");
        }

        print!("{}", String::from_utf8_lossy(&output.stdout));
    }

    Ok(())
}
