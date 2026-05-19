use anyhow::{Context, Result};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use super::registry::MarketplaceRegistry;

/// Trait boundary for registry I/O.
///
/// The only place `reqwest` and `tokio::fs` are allowed under `src/marketplace/`.
pub trait RegistryLoader: Send + Sync {
    /// Fetch a registry from a remote URL.
    fn fetch<'a>(&'a self, url: &'a str) -> Pin<Box<dyn Future<Output = Result<MarketplaceRegistry>> + Send + 'a>>;

    /// Fetch a registry from a local file path.
    fn fetch_file<'a>(
        &'a self,
        path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<MarketplaceRegistry>> + Send + 'a>>;
}

/// Production loader backed by `reqwest` and `tokio::fs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReqwestRegistryLoader;

impl RegistryLoader for ReqwestRegistryLoader {
    fn fetch<'a>(&'a self, url: &'a str) -> Pin<Box<dyn Future<Output = Result<MarketplaceRegistry>> + Send + 'a>> {
        Box::pin(async move {
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

            response
                .json::<MarketplaceRegistry>()
                .await
                .with_context(|| format!("Failed to parse registry JSON from {}", url))
        })
    }

    fn fetch_file<'a>(
        &'a self,
        path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<MarketplaceRegistry>> + Send + 'a>> {
        Box::pin(async move {
            let content = tokio::fs::read_to_string(path)
                .await
                .with_context(|| format!("Failed to read registry file {}", path.display()))?;
            serde_json::from_str(&content).context("Failed to parse registry JSON")
        })
    }
}

/// In-memory loader for unit tests.
#[derive(Default, Debug)]
pub struct MockRegistryLoader {
    pub registries: std::sync::Mutex<std::collections::HashMap<String, MarketplaceRegistry>>,
}

impl RegistryLoader for MockRegistryLoader {
    fn fetch<'a>(&'a self, url: &'a str) -> Pin<Box<dyn Future<Output = Result<MarketplaceRegistry>> + Send + 'a>> {
        let url = url.to_string();
        Box::pin(async move {
            self.registries
                .lock()
                .expect("mock registry lock")
                .get(&url)
                .cloned()
                .context(format!("Mock registry not found: {}", url))
        })
    }

    fn fetch_file<'a>(
        &'a self,
        path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<MarketplaceRegistry>> + Send + 'a>> {
        let key = path.display().to_string();
        Box::pin(async move {
            self.registries
                .lock()
                .expect("mock registry lock")
                .get(&key)
                .cloned()
                .context(format!("Mock registry not found: {}", key))
        })
    }
}
