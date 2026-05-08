use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// An agent role defined in AGENTS.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRole {
    pub role: String,
    pub description: String,
    #[serde(default)]
    pub tier: Option<String>,
}

/// Parsed AGENTS.md manifest.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentsManifest {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub agents: Vec<AgentRole>,
    #[serde(default)]
    pub body: String,
}

/// Parse an AGENTS.md file with YAML frontmatter.
pub fn parse_agents_md(content: &str) -> Result<AgentsManifest> {
    let (frontmatter, body) = split_frontmatter(content)?;
    let mut manifest: AgentsManifest = if frontmatter.is_empty() {
        AgentsManifest::default()
    } else {
        serde_yaml::from_str(&frontmatter)
            .with_context(|| "Failed to parse AGENTS.md YAML frontmatter")?
    };
    manifest.body = body.trim().to_string();
    Ok(manifest)
}

/// Load AGENTS.md from a directory.
pub async fn load_agents_file(dir: &Path) -> Result<Option<AgentsManifest>> {
    let path = dir.join("AGENTS.md");
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read {}", path.display()))?;
    parse_agents_md(&content).map(Some)
}

fn split_frontmatter(content: &str) -> Result<(String, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok((String::new(), content.to_string()));
    }

    let after_open = &trimmed[3..];
    if let Some(close_idx) = after_open.find("---") {
        let frontmatter = after_open[..close_idx].trim();
        let body = after_open[close_idx + 3..].to_string();
        Ok((frontmatter.to_string(), body))
    } else {
        anyhow::bail!("AGENTS.md has opening --- but no closing ---");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agents_md() {
        let content = r#"---
name: My Project
description: A test project
agents:
  - role: architect
    description: Designs systems
  - role: frontend
    description: Builds UI
---
# Context

This project uses Rust and React.
"#;
        let manifest = parse_agents_md(content).unwrap();
        assert_eq!(manifest.name, Some("My Project".to_string()));
        assert_eq!(manifest.agents.len(), 2);
        assert_eq!(manifest.agents[0].role, "architect");
        assert!(manifest.body.contains("Rust and React"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        let manifest = parse_agents_md(content).unwrap();
        assert!(manifest.name.is_none());
        assert!(manifest.agents.is_empty());
        assert!(manifest.body.contains("Just markdown"));
    }
}
