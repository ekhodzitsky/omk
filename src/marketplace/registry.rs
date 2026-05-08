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
pub async fn load_all_skills(
    registries: &[String],
) -> Result<Vec<(String, RegistrySkill)>> {
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
