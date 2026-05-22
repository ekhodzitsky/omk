use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    command: MarketplaceCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum MarketplaceCommands {
    /// List available skills in the marketplace
    List {
        /// Use a specific registry URL instead of configured ones
        #[arg(long)]
        registry: Option<String>,
    },
    /// Install a skill from the marketplace
    Install {
        /// Skill name or index
        name: String,
        /// Use a specific registry URL
        #[arg(long)]
        registry: Option<String>,
    },
    /// Search for skills across all registries
    Search {
        /// Search query
        query: String,
    },
    /// Show detailed info about a skill
    Info {
        /// Skill name
        name: String,
    },
    /// Add an external registry URL
    AddRegistry {
        /// Registry URL (http/https or local file path)
        url: String,
    },
    /// Remove a registry URL
    RemoveRegistry {
        /// Registry URL to remove
        url: String,
    },
    /// List configured registries
    ListRegistries,
}

#[allow(dead_code)]
struct MarketSkill {
    name: &'static str,
    description: &'static str,
    url: &'static str,
    author: &'static str,
    tags: &'static [&'static str],
}

const MARKET_SKILLS: &[MarketSkill] = &[
    MarketSkill {
        name: "rust-expert",
        description: "Advanced Rust patterns, unsafe guidelines, async best practices",
        url: "https://github.com/ekhodzitsky/omk-skill-rust",
        author: "@ekhodzitsky",
        tags: &["rust", "systems"],
    },
    MarketSkill {
        name: "web-dev",
        description: "Full-stack web development with React, Node, and TypeScript",
        url: "https://github.com/ekhodzitsky/omk-skill-web",
        author: "@ekhodzitsky",
        tags: &["web", "typescript", "react"],
    },
    MarketSkill {
        name: "devops",
        description: "Docker, Kubernetes, CI/CD pipelines, infrastructure as code",
        url: "https://github.com/ekhodzitsky/omk-skill-devops",
        author: "@ekhodzitsky",
        tags: &["devops", "docker", "k8s"],
    },
    MarketSkill {
        name: "security",
        description: "Security audit patterns, vulnerability assessment, secure coding",
        url: "https://github.com/ekhodzitsky/omk-skill-security",
        author: "@ekhodzitsky",
        tags: &["security", "audit"],
    },
];

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        MarketplaceCommands::List { registry } => list_skills(registry).await,
        MarketplaceCommands::Install { name, registry } => install_skill(&name, registry).await,
        MarketplaceCommands::Search { query } => search_skills(&query).await,
        MarketplaceCommands::Info { name } => info_skill(&name).await,
        MarketplaceCommands::AddRegistry { url } => add_registry(&url).await,
        MarketplaceCommands::RemoveRegistry { url } => remove_registry(&url).await,
        MarketplaceCommands::ListRegistries => list_registries().await,
    }
}

async fn list_skills(registry_override: Option<String>) -> Result<()> {
    println!("🛒 omk Marketplace\n");

    // Built-in skills
    println!("Built-in skills:");
    println!("{:<4} {:<20} {:<50} Tags", "#", "Name", "Description");
    println!("{}", "─".repeat(100));
    for (i, skill) in MARKET_SKILLS.iter().enumerate() {
        let tags = skill.tags.join(", ");
        println!(
            "{:<4} {:<20} {:<50} {}",
            i + 1,
            skill.name,
            skill.description.chars().take(48).collect::<String>(),
            tags
        );
    }

    // External registries
    let config = crate::runtime::config::load_config().await?;
    let registries = if let Some(url) = registry_override {
        vec![url]
    } else {
        config.registries.clone()
    };

    if !registries.is_empty() {
        println!("\nExternal registries:");
        match crate::marketplace::load_all_skills(&registries).await {
            Ok(skills) => {
                if skills.is_empty() {
                    println!("  (no skills found)");
                } else {
                    for (i, (registry_name, skill)) in skills.iter().enumerate() {
                        let tags = skill.tags.join(", ");
                        println!(
                            "  {:<4} {:<20} {:<40} {} (from {})",
                            i + 1,
                            skill.name,
                            skill.description.chars().take(38).collect::<String>(),
                            tags,
                            registry_name
                        );
                    }
                }
            }
            Err(e) => {
                println!("  Error loading registries: {}", e);
            }
        }
    }

    println!("\nInstall with: omk marketplace install <name>");
    Ok(())
}

async fn install_skill(name: &str, registry_override: Option<String>) -> Result<()> {
    // Try built-in first
    let builtin = if let Ok(idx) = name.parse::<usize>() {
        MARKET_SKILLS.get(idx.saturating_sub(1))
    } else {
        MARKET_SKILLS.iter().find(|s| s.name == name)
    };

    if let Some(skill) = builtin {
        println!("Installing '{}' from built-in marketplace...", skill.name);
        return crate::cli::skill::run(crate::cli::skill::Args {
            command: crate::cli::skill::SkillCommands::Install {
                url: skill.url.to_string(),
                name: Some(skill.name.to_string()),
            },
        })
        .await;
    }

    // Try external registries
    let config = crate::runtime::config::load_config().await?;
    let registries = if let Some(url) = registry_override {
        vec![url]
    } else {
        config.registries.clone()
    };

    if !registries.is_empty() {
        let skills = crate::marketplace::load_all_skills(&registries).await?;
        if let Some((_, skill)) = skills.into_iter().find(|(_, s)| s.name == name) {
            println!("Installing '{}' from external registry...", skill.name);
            return crate::cli::skill::run(crate::cli::skill::Args {
                command: crate::cli::skill::SkillCommands::Install {
                    url: skill.url,
                    name: Some(skill.name),
                },
            })
            .await;
        }
    }

    anyhow::bail!(
        "Skill '{}' not found in marketplace or configured registries",
        name
    )
}

async fn search_skills(query: &str) -> Result<()> {
    let query_lower = query.to_lowercase();

    // Search built-in
    let builtin_matches: Vec<_> = MARKET_SKILLS
        .iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&query_lower)
                || s.description.to_lowercase().contains(&query_lower)
                || s.tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
        })
        .collect();

    // Search external
    let config = crate::runtime::config::load_config().await?;
    let external_matches = if !config.registries.is_empty() {
        match crate::marketplace::load_all_skills(&config.registries).await {
            Ok(skills) => skills
                .into_iter()
                .filter(|(_, s)| {
                    s.name.to_lowercase().contains(&query_lower)
                        || s.description.to_lowercase().contains(&query_lower)
                        || s.tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&query_lower))
                })
                .collect(),
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

    let total = builtin_matches.len() + external_matches.len();
    if total == 0 {
        println!("No skills found for '{}'", query);
        return Ok(());
    }

    println!("Found {} skill(s) for '{}':\n", total, query);

    for (i, skill) in builtin_matches.iter().enumerate() {
        println!(
            "  {}. {} — {} [built-in]",
            i + 1,
            skill.name,
            skill.description
        );
    }
    for (i, (registry, skill)) in external_matches.iter().enumerate() {
        println!(
            "  {}. {} — {} [{}]",
            builtin_matches.len() + i + 1,
            skill.name,
            skill.description,
            registry
        );
    }

    Ok(())
}

async fn info_skill(name: &str) -> Result<()> {
    // Try built-in first
    if let Some(skill) = MARKET_SKILLS.iter().find(|s| s.name == name) {
        println!("Skill: {} [built-in]", skill.name);
        println!("Description: {}", skill.description);
        println!("Author: {}", skill.author);
        println!("URL: {}", skill.url);
        println!("Tags: {}", skill.tags.join(", "));
        return Ok(());
    }

    // Try external registries
    let config = crate::runtime::config::load_config().await?;
    if !config.registries.is_empty() {
        let skills = crate::marketplace::load_all_skills(&config.registries).await?;
        if let Some((registry, skill)) = skills.into_iter().find(|(_, s)| s.name == name) {
            println!("Skill: {}", skill.name);
            println!("Registry: {}", registry);
            println!("Description: {}", skill.description);
            println!("Author: {}", skill.author);
            println!("URL: {}", skill.url);
            println!("Tags: {}", skill.tags.join(", "));
            return Ok(());
        }
    }

    anyhow::bail!(
        "Skill '{}' not found in marketplace or configured registries",
        name
    )
}

async fn add_registry(url: &str) -> Result<()> {
    // Validate URL format
    if !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with('/')
        && !url.starts_with("./")
    {
        anyhow::bail!(
            "Invalid registry URL '{}'. Must start with http://, https://, or be an absolute/relative file path",
            url
        );
    }

    let mut config = crate::runtime::config::load_config().await?;

    if config.registries.contains(&url.to_string()) {
        println!("Registry '{}' is already configured.", url);
        return Ok(());
    }

    // Validate by fetching
    println!("Validating registry '{}'...", url);
    let _ = if url.starts_with("http://") || url.starts_with("https://") {
        crate::marketplace::MarketplaceRegistry::fetch(url).await
    } else {
        crate::marketplace::MarketplaceRegistry::fetch_file(std::path::Path::new(url)).await
    }
    .context(
        "Failed to validate registry. Make sure the URL is accessible and returns valid JSON.",
    )?;

    config.registries.push(url.to_string());

    let config_path = crate::runtime::config::config_dir().join("config.toml");
    let content = toml::to_string_pretty(&config).context("Failed to serialize config")?;
    crate::runtime::atomic::atomic_write(&config_path, content.as_bytes()).await?;

    println!("✓ Added registry '{}'", url);
    Ok(())
}

async fn remove_registry(url: &str) -> Result<()> {
    let mut config = crate::runtime::config::load_config().await?;

    let before = config.registries.len();
    config.registries.retain(|r| r != url);

    if config.registries.len() == before {
        println!("Registry '{}' was not found in configuration.", url);
        return Ok(());
    }

    let config_path = crate::runtime::config::config_dir().join("config.toml");
    let content = toml::to_string_pretty(&config).context("Failed to serialize config")?;
    crate::runtime::atomic::atomic_write(&config_path, content.as_bytes()).await?;

    println!("✓ Removed registry '{}'", url);
    Ok(())
}

async fn list_registries() -> Result<()> {
    let config = crate::runtime::config::load_config().await?;

    if config.registries.is_empty() {
        println!("No external registries configured.");
        println!("Add one with: omk marketplace add-registry <url>");
        return Ok(());
    }

    println!("Configured registries:\n");
    for (i, url) in config.registries.iter().enumerate() {
        println!("  {}. {}", i + 1, url);
    }

    Ok(())
}
