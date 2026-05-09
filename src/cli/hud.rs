use anyhow::Result;
use clap::Parser;

/// HUD / statusline
#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
    /// Team name to monitor
    pub team_name: Option<String>,

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

    /// Print single snapshot and exit
    #[arg(long)]
    pub once: bool,

    /// Output JSON instead of text
    #[arg(long)]
    pub json: bool,
}

pub(crate) async fn run(args: Args) -> Result<()> {
    if args.tui {
        if let Some(team_name) = args.team_name {
            let state_dir = crate::runtime::config::omk_state_dir()
                .join("team")
                .join(&team_name);

            if !state_dir.exists() {
                anyhow::bail!(
                    "Team '{}' not found. Expected state at: {}",
                    team_name,
                    state_dir.display()
                );
            }

            #[cfg(feature = "tui")]
            {
                let mut hud_tui = crate::vis::hud_tui::HudTui::new(&team_name, state_dir);
                hud_tui.run().await?;
            }
            #[cfg(not(feature = "tui"))]
            {
                anyhow::bail!("TUI requires --features tui");
            }
        } else {
            println!("omk hud <team_name> --tui");
            println!("  --tui requires a team name to monitor");
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
        if let Some(team_name) = args.team_name {
            run_team_hud_tmux(team_name).await?;
        } else {
            let status = generate_status_bar().await?;
            println!("{}", status);
        }
    } else if let Some(team_name) = args.team_name {
        run_team_hud(team_name, args.once, args.json).await?;
    } else {
        println!("omk hud");
        println!("  <team_name>  Monitor a specific team");
        println!("  --tmux       Output tmux status bar string");
        println!("  --tui        Run interactive TUI");
        println!("  --web        Start web dashboard");
        println!("  --port       Port for web dashboard (default: 8080)");
        println!("  --once       Print single snapshot and exit");
        println!("  --json       Output JSON instead of text");
    }
    Ok(())
}

async fn run_team_hud(team_name: String, once: bool, json: bool) -> Result<()> {
    let state_dir = crate::runtime::config::omk_state_dir()
        .join("team")
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let events_path = state_dir.join("events.jsonl");
    let mut event_stream = crate::vis::event_stream::EventStream::new(&events_path);
    let config = crate::runtime::watchdog::WatchdogConfig {
        require_tmux: false,
        ..Default::default()
    };
    let watchdog = crate::runtime::watchdog::Watchdog::new(config);

    let run_id = team_name.clone();
    let mut hud_state = crate::vis::hud::HudState::new(&team_name, &run_id);

    // Initial refresh
    hud_state
        .refresh(&mut event_stream, &watchdog, &state_dir)
        .await?;

    if once {
        if json {
            println!("{}", hud_state.render_json()?);
        } else {
            println!("{}", hud_state.render_text());
        }
        return Ok(());
    }

    // Loop with Ctrl+C handling
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                hud_state
                    .refresh(&mut event_stream, &watchdog, &state_dir)
                    .await?;

                // Clear screen and re-render
                print!("\x1B[2J\x1B[H"); // ANSI clear screen + home cursor
                if json {
                    println!("{}", hud_state.render_json()?);
                } else {
                    println!("{}", hud_state.render_text());
                }

                // Check if run is complete
                let is_complete = hud_state.events.iter().any(|e| {
                    matches!(
                        e.kind,
                        crate::runtime::events::EventKind::RunCompleted
                            | crate::runtime::events::EventKind::RunFailed
                            | crate::runtime::events::EventKind::ManualInterrupt
                    )
                });

                if is_complete {
                    println!("\nRun complete. Exiting.");
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nExiting HUD.");
                break;
            }
        }
    }

    Ok(())
}

async fn run_team_hud_tmux(team_name: String) -> Result<()> {
    let state_dir = crate::runtime::config::omk_state_dir()
        .join("team")
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let events_path = state_dir.join("events.jsonl");
    let mut event_stream = crate::vis::event_stream::EventStream::new(&events_path);
    let watchdog = crate::runtime::watchdog::Watchdog::with_defaults();

    let run_id = team_name.clone();
    let mut hud_state = crate::vis::hud::HudState::new(&team_name, &run_id);
    hud_state
        .refresh(&mut event_stream, &watchdog, &state_dir)
        .await?;
    println!("{}", hud_state.to_tmux_status());
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
