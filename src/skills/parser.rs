use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub level: Option<u8>,
    pub aliases: Vec<String>,
    pub triggers: Vec<String>,
    pub body: String,
    pub source_path: std::path::PathBuf,
}

/// Parse a SKILL.md file with YAML frontmatter
pub fn parse_skill(path: &Path) -> Result<Skill> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read skill file: {}", path.display()))?;

    let (frontmatter, body) = extract_frontmatter(&content)?;

    let meta: SkillMeta = serde_yaml::from_str(&frontmatter)
        .with_context(|| format!("Invalid YAML frontmatter in {}", path.display()))?;

    Ok(Skill {
        name: meta.name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        }),
        description: meta.description.unwrap_or_else(|| {
            body.lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("No description")
                .to_string()
        }),
        level: meta.level,
        aliases: meta.aliases.unwrap_or_default(),
        triggers: meta.triggers.unwrap_or_default(),
        body: body.to_string(),
        source_path: path.to_path_buf(),
    })
}

fn extract_frontmatter(content: &str) -> Result<(String, String)> {
    lazy_static::lazy_static! {
        static ref RE: Regex = Regex::new(
            r"(?s)^---\s*\n(.*?)\n---\s*\n?(.*)$"
        ).unwrap();
    }

    if let Some(caps) = RE.captures(content) {
        let fm = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let body = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
        Ok((fm, body))
    } else {
        // No frontmatter — treat entire content as body
        Ok((String::new(), content.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct SkillMeta {
    name: Option<String>,
    description: Option<String>,
    level: Option<u8>,
    aliases: Option<Vec<String>>,
    triggers: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let input = "---\nname: team\ndescription: N coordinated agents\nlevel: 4\naliases: [\"tm\", \"swarm\"]\ntriggers: [\"team\", \"orchestrate\"]\n---\n\n# Team Mode\n\nUse this when coordinating multiple agents.\n";
        let (fm, body) = extract_frontmatter(input).unwrap();
        eprintln!("FM: {:?}", fm);
        eprintln!("BODY: {:?}", body);
        assert!(fm.contains("name: team"), "frontmatter missing name: {:?}", fm);
        assert!(body.contains("# Team Mode"), "body missing header: {:?}", body);
    }
}
