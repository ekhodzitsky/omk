use anyhow::Result;
use clap::Parser;

/// HUD / statusline
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Render tmux status bar string
    #[arg(long)]
    pub tmux: bool,

    /// Show live TUI
    #[arg(long)]
    pub tui: bool,

    /// Start web dashboard
    #[arg(long)]
    pub web: bool,

    /// Port for web dashboard
    #[arg(long, default_value = "8080")]
    pub port: u16,
}

pub async fn run(args: Args) -> Result<()> {
    if args.tui {
        #[cfg(feature = "tui")]
        {
            crate::vis::hud::run_tui().await?;
        }
        #[cfg(not(feature = "tui"))]
        {
            anyhow::bail!("TUI feature is not enabled. Rebuild with --features tui");
        }
    } else if args.web {
        #[cfg(feature = "server")]
        {
            crate::vis::server::run_server(args.port).await?;
        }
        #[cfg(not(feature = "server"))]
        {
            anyhow::bail!("Server feature is not enabled. Rebuild with --features server");
        }
    } else if args.tmux {
        let status = generate_status_bar().await?;
        println!("{}", status);
    } else {
        println!("omk hud");
        println!("  --tmux   Output tmux status bar string");
        println!("  --tui    Run interactive TUI");
        println!("  --web    Start web dashboard");
        println!("  --port   Port for web dashboard (default: 8080)");
    }
    Ok(())
}

async fn generate_status_bar() -> Result<String> {
    // TODO: Read .omk/state/ for active teams, ralph modes, token usage
    let mut parts = vec!["omk".to_string()];

    let omk_dir = Some(crate::runtime::config::omk_state_dir()).filter(|p| p.exists());

    if let Some(state_dir) = omk_dir {
        let mut read = tokio::fs::read_dir(&state_dir).await?;
        while let Some(entry) = read.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("team-") || name.starts_with("ralph-") {
                parts.push(name);
            }
        }
    }

    Ok(parts.join(" | "))
}
