use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: ConfigCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCommands {
    /// Validate config.toml and environment
    Validate,
    /// Show current config paths and values
    Show,
    /// Set a configuration value
    Set {
        /// Key to set (e.g. default_team_size, default_yolo)
        key: String,
        /// Value to set
        value: String,
    },
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        ConfigCommands::Validate => validate().await,
        ConfigCommands::Show => show().await,
        ConfigCommands::Set { key, value } => set(&key, &value).await,
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
            println!(
                "  Legacy dir ............ ⚠ {} still exists. Consider migrating to XDG dirs.",
                l.display()
            );
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
    let config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();

    println!("omk Configuration");
    println!("=================\n");

    println!("Paths:");
    println!(
        "  Config: {}",
        crate::runtime::config::config_dir().display()
    );
    println!(
        "  State:  {}",
        crate::runtime::config::state_dir().display()
    );
    println!("  Data:   {}", crate::runtime::config::data_dir().display());
    println!(
        "  Cache:  {}",
        crate::runtime::config::cache_dir().display()
    );
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
    if !config.registries.is_empty() {
        println!("  registries:");
        for url in &config.registries {
            println!("    - {}", url);
        }
    }

    Ok(())
}

async fn set(key: &str, value: &str) -> Result<()> {
    let mut config = crate::runtime::config::load_config()
        .await
        .unwrap_or_default();

    match key {
        "default_team_size" => {
            let size: usize = value
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid number for default_team_size: {}", e))?;
            if size == 0 || size > 16 {
                anyhow::bail!("default_team_size must be between 1 and 16");
            }
            config.default_team_size = size;
        }
        "default_yolo" => {
            config.default_yolo = value
                .parse::<bool>()
                .map_err(|e| anyhow::anyhow!("Invalid boolean for default_yolo: {}", e))?;
        }
        "enable_metrics" => {
            config.enable_metrics = value
                .parse::<bool>()
                .map_err(|e| anyhow::anyhow!("Invalid boolean for enable_metrics: {}", e))?;
        }
        "kimi_binary" => {
            let path = std::path::PathBuf::from(value);
            if !path.exists() {
                anyhow::bail!("kimi_binary path does not exist: {}", path.display());
            }
            config.kimi_binary = Some(value.to_string());
        }
        _ => {
            anyhow::bail!("Unknown config key: {}. Known keys: default_team_size, default_yolo, enable_metrics, kimi_binary", key);
        }
    }

    let config_dir = crate::runtime::config::config_dir();
    crate::runtime::config::ensure_private_dir(&config_dir).await?;
    let config_path = config_dir.join("config.toml");
    let content = toml::to_string_pretty(&config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    crate::runtime::atomic::atomic_write(&config_path, content.as_bytes()).await?;

    println!("✓ Set {} = {}", key, value);
    Ok(())
}

async fn check_dir(path: &std::path::Path) -> Result<()> {
    crate::runtime::config::ensure_private_dir(path).await?;
    let test = path.join(".omk-write-test");
    tokio::fs::write(&test, b"x").await?;
    tokio::fs::remove_file(&test).await?;
    Ok(())
}
