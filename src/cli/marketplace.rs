use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    command: MarketplaceCommands,
}

#[derive(Subcommand, Debug)]
pub enum MarketplaceCommands {
    /// List available skills in the marketplace
    List,
    /// Install a skill from the marketplace
    Install {
        /// Skill name or index
        name: String,
    },
    /// Search for skills (placeholder for future registry)
    Search {
        /// Search query
        query: String,
    },
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

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        MarketplaceCommands::List => list_skills().await,
        MarketplaceCommands::Install { name } => install_skill(&name).await,
        MarketplaceCommands::Search { query } => search_skills(&query).await,
    }
}

async fn list_skills() -> Result<()> {
    println!("🛒 omk Marketplace");
    println!();
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

    println!();
    println!("Install with: omk marketplace install <name>");
    Ok(())
}

async fn install_skill(name: &str) -> Result<()> {
    let skill = if let Ok(idx) = name.parse::<usize>() {
        MARKET_SKILLS.get(idx.saturating_sub(1))
    } else {
        MARKET_SKILLS.iter().find(|s| s.name == name)
    };

    let skill = skill.ok_or_else(|| anyhow::anyhow!("Skill '{}' not found in marketplace", name))?;

    println!("Installing '{}' from {}...", skill.name, skill.url);

    // Delegate to skill install
    crate::cli::skill::run(crate::cli::skill::Args {
        command: crate::cli::skill::SkillCommands::Install {
            url: skill.url.to_string(),
            name: Some(skill.name.to_string()),
        },
    }).await
}

async fn search_skills(query: &str) -> Result<()> {
    let query_lower = query.to_lowercase();
    let matches: Vec<_> = MARKET_SKILLS
        .iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&query_lower)
                || s.description.to_lowercase().contains(&query_lower)
                || s.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
        })
        .collect();

    if matches.is_empty() {
        println!("No skills found for '{}'", query);
        return Ok(());
    }

    println!("Found {} skill(s) for '{}':", matches.len(), query);
    println!();
    for (i, skill) in matches.iter().enumerate() {
        println!("  {}. {} — {}", i + 1, skill.name, skill.description);
    }

    Ok(())
}
