use anyhow::Result;
use tracing::info;

pub(super) async fn run_setup() -> Result<()> {
    info!("Running omk setup");

    crate::runtime::config::ensure_dirs().await?;

    let config_dir = crate::runtime::config::config_dir();
    let state_dir = crate::runtime::config::state_dir();
    let data_dir = crate::runtime::config::data_dir();

    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = r#"# OMK Configuration
# See https://github.com/ekhodzitsky/omk for docs

# Default number of workers for team mode
default_team_size = 2

# Enable YOLO (auto-approve) mode by default
default_yolo = false

# Path to Kimi CLI binary (leave empty for auto-detect)
# kimi_binary = "/usr/local/bin/kimi"

# Additional skill directories
# extra_skill_dirs = ["~/.omk/skills"]

# Enable metrics collection
enable_metrics = true
"#;
        crate::runtime::atomic::atomic_write(&config_path, default_config.as_bytes()).await?;
    }

    let skills_dir = data_dir.join("skills");
    tokio::fs::create_dir_all(&skills_dir).await?;

    let project_omk = std::env::current_dir()?.join(".omk");
    tokio::fs::create_dir_all(&project_omk).await.ok();
    let agents_path = project_omk.join("AGENTS.md");
    if !agents_path.exists() {
        tokio::fs::write(&agents_path, crate::agents::runtime::default_agents_md()).await?;
        println!("✓ Created {}", agents_path.display());
    }

    println!("✓ omk setup complete");
    println!("  Config: {}", config_dir.display());
    println!("  State:  {}", state_dir.display());
    println!("  Data:   {}", data_dir.display());
    println!();
    println!("Next steps:");
    println!("  1. Ensure 'kimi' CLI is installed and authenticated");
    println!("  2. Run 'omk team run 2:coder \"fix TypeScript errors\"' to try team mode");

    Ok(())
}
