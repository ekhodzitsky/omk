use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A skill entry in an external marketplace registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySkill {
    pub name: String,
    pub description: String,
    pub author: String,
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// External marketplace registry format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceRegistry {
    pub name: String,
    pub url: String,
    pub skills: Vec<RegistrySkill>,
}

impl MarketplaceRegistry {
    /// Fetch and parse a registry from a URL.
    pub async fn fetch(url: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch registry from {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Registry returned HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        let registry: MarketplaceRegistry = response
            .json()
            .await
            .with_context(|| format!("Failed to parse registry JSON from {}", url))?;

        Ok(registry)
    }

    /// Fetch from a local file path.
    pub async fn fetch_file(path: &std::path::Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read registry file {}", path.display()))?;
        let registry: MarketplaceRegistry =
            serde_json::from_str(&content).context("Failed to parse registry JSON")?;
        Ok(registry)
    }
}

/// Load all skills from configured registries plus the built-in list.
pub async fn load_all_skills(registries: &[String]) -> Result<Vec<(String, RegistrySkill)>> {
    let mut all = Vec::new();

    for url in registries {
        let registry = if url.starts_with("http://") || url.starts_with("https://") {
            MarketplaceRegistry::fetch(url).await
        } else {
            MarketplaceRegistry::fetch_file(std::path::Path::new(url)).await
        };

        match registry {
            Ok(r) => {
                for skill in r.skills {
                    all.push((r.name.clone(), skill));
                }
            }
            Err(e) => {
                tracing::warn!(registry = %url, error = %e, "Failed to load marketplace registry");
            }
        }
    }

    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_deserialization_roundtrip() {
        let registry = MarketplaceRegistry {
            name: "test-registry".to_string(),
            url: "https://example.com/registry.json".to_string(),
            skills: vec![
                RegistrySkill {
                    name: "skill-a".to_string(),
                    description: "Does A".to_string(),
                    author: "alice".to_string(),
                    url: "https://example.com/skill-a".to_string(),
                    tags: vec!["rust".to_string(), "cli".to_string()],
                },
                RegistrySkill {
                    name: "skill-b".to_string(),
                    description: "Does B".to_string(),
                    author: "bob".to_string(),
                    url: "https://example.com/skill-b".to_string(),
                    tags: vec![],
                },
            ],
        };

        let json = serde_json::to_string(&registry).unwrap();
        let parsed: MarketplaceRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "test-registry");
        assert_eq!(parsed.skills.len(), 2);
        assert_eq!(parsed.skills[0].name, "skill-a");
        assert_eq!(parsed.skills[0].tags, vec!["rust", "cli"]);
        assert_eq!(parsed.skills[1].tags, Vec::<String>::new());
    }

    #[test]
    fn registry_skill_deserialization_with_missing_tags() {
        let json = r#"{
            "name": "skill-c",
            "description": "Does C",
            "author": "charlie",
            "url": "https://example.com/skill-c"
        }"#;
        let skill: RegistrySkill = serde_json::from_str(json).unwrap();
        assert_eq!(skill.name, "skill-c");
        assert!(skill.tags.is_empty());
    }

    #[test]
    fn registry_deserialization_with_empty_skills() {
        let json = r#"{
            "name": "empty-registry",
            "url": "https://example.com/empty.json",
            "skills": []
        }"#;
        let registry: MarketplaceRegistry = serde_json::from_str(json).unwrap();
        assert!(registry.skills.is_empty());
    }

    #[tokio::test]
    async fn load_all_skills_empty_registries() {
        let result = load_all_skills(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn load_all_skills_skips_unreachable_registries() {
        // A non-existent local path should be skipped gracefully.
        let result = load_all_skills(&["/nonexistent/path/registry.json".to_string()])
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn registry_skill_equality_and_clone() {
        let skill = RegistrySkill {
            name: "skill".to_string(),
            description: "desc".to_string(),
            author: "auth".to_string(),
            url: "https://example.com".to_string(),
            tags: vec!["t".to_string()],
        };
        let cloned = skill.clone();
        assert_eq!(skill.name, cloned.name);
        assert_eq!(skill.description, cloned.description);
    }
}
