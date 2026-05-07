use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    command: ConfigCommands,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Validate config.toml and environment
    Validate,
    /// Show current config paths and values
    Show,
}

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        ConfigCommands::Validate => validate().await,
        ConfigCommands::Show => show().await,
    }
}

async fn validate() -> Result<()> {
    println!("🔍 Validating omk configuration...\n");

    let mut issues = 0;

    // Check config file exists and is valid
    let config_dir = crate::runtime::config::config_dir();
    let config_path = config_dir.join("config.toml");
    print!("  Config file ........... ");
    if config_path.exists() {
        match tokio::fs::read_to_string(&config_path).await {
            Ok(content) => {
                match crate::runtime::config::load_config().await {
                    Ok(config) => {
                        println!("✓ {} (valid)", config_path.display());
                        if config.default_team_size == 0 || config.default_team_size > 16 {
                            println!("    ⚠ default_team_size should be between 1 and 16");
                            issues += 1;
                        }
                    }
                    Err(e) => {
                        println!("✗ Invalid TOML: {}", e);
                        issues += 1;
                    }
                }
                if content.contains("~/.omk") {
                    println!("    ⚠ Config references legacy ~/.omk paths. Consider migrating to XDG dirs.");
                }
            }
            Err(e) => {
                println!("✗ Cannot read: {}", e);
                issues += 1;
            }
        }
    } else {
        println!("⚠ Not found. Run `omk setup` to create default config.");
    }

    // Check directories
    let dirs = [
        ("Config dir", crate::runtime::config::config_dir()),
        ("State dir", crate::runtime::config::state_dir()),
        ("Data dir", crate::runtime::config::data_dir()),
        ("Cache dir", crate::runtime::config::cache_dir()),
    ];

    for (name, path) in &dirs {
        print!("  {:22} ", format!("{} ...", name));
        match check_dir(path).await {
            Ok(_) => println!("✓ {}", path.display()),
            Err(e) => {
                println!("✗ {}", e);
                issues += 1;
            }
        }
    }

    // Check for legacy ~/.omk
    let legacy = dirs::home_dir().map(|h| h.join(".omk"));
    if let Some(ref l) = legacy {
        if l.exists() {
            println!("  Legacy dir ............ ⚠ {} still exists. Consider migrating to XDG dirs.", l.display());
        }
    }

    println!();
    if issues == 0 {
        println!("✅ Configuration is valid.");
    } else {
        println!("⚠️  Found {} issue(s).", issues);
    }

    Ok(())
}

async fn show() -> Result<()> {
    let config = crate::runtime::config::load_config().await.unwrap_or_default();

    println!("omk Configuration");
    println!("=================\n");

    println!("Paths:");
    println!("  Config: {}", crate::runtime::config::config_dir().display());
    println!("  State:  {}", crate::runtime::config::state_dir().display());
    println!("  Data:   {}", crate::runtime::config::data_dir().display());
    println!("  Cache:  {}", crate::runtime::config::cache_dir().display());
    println!();

    println!("Settings:");
    println!("  default_team_size: {}", config.default_team_size);
    println!("  default_yolo:      {}", config.default_yolo);
    println!("  enable_metrics:    {}", config.enable_metrics);
    if let Some(ref bin) = config.kimi_binary {
        println!("  kimi_binary:       {}", bin);
    } else {
        println!("  kimi_binary:       (auto-detect)");
    }
    if !config.extra_skill_dirs.is_empty() {
        println!("  extra_skill_dirs:");
        for dir in &config.extra_skill_dirs {
            println!("    - {}", dir.display());
        }
    }

    Ok(())
}

async fn check_dir(path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        tokio::fs::create_dir_all(path).await?;
    }
    let test = path.join(".omk-write-test");
    tokio::fs::write(&test, b"x").await?;
    tokio::fs::remove_file(&test).await?;
    Ok(())
}
