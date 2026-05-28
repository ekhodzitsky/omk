use anyhow::Result;
use tracing::{info, warn};

use super::artifact::save_artifact;
use super::execution::run_advisor_direct;
use super::provider::available_providers;

/// Query a single provider and optionally persist the artifact.
pub async fn ask_single(
    provider: &str,
    prompt: &str,
    save: bool,
    timeout_secs: u64,
) -> Result<String> {
    let output = run_advisor_direct(provider, prompt, timeout_secs).await?;

    if save {
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let content = format!("# {provider} answer\n\nPrompt: {prompt}\n\n---\n\n{output}\n");
        save_artifact(provider, &content, &ts).await?;
    }

    Ok(output)
}

/// Query specific providers in parallel.
/// Returns a vec of `(provider, output)` pairs.
pub async fn ask_providers(
    providers: &[&str],
    prompt: &str,
    save: bool,
    timeout_secs: u64,
) -> Result<Vec<(String, String)>> {
    if providers.is_empty() {
        anyhow::bail!("No providers specified");
    }

    info!(providers = ?providers, "Querying advisors");
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();

    let mut tasks = tokio::task::JoinSet::new();
    for &provider in providers {
        let p = prompt.to_string();
        let t = timeout_secs;
        let provider_owned = provider.to_string();
        tasks.spawn(async move {
            match run_advisor_direct(&provider_owned, &p, t).await {
                Ok(output) => Ok((provider_owned, output)),
                Err(e) => Err((provider_owned, e)),
            }
        });
    }

    let mut results = Vec::new();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok((provider, output))) => {
                if save {
                    let content =
                        format!("# {provider} answer\n\nPrompt: {prompt}\n\n---\n\n{output}\n");
                    if let Err(e) = save_artifact(&provider, &content, &ts).await {
                        warn!(provider = provider, error = %e, "Failed to save advisor artifact");
                    }
                }
                results.push((provider, output));
            }
            Ok(Err((provider, e))) => {
                warn!(provider = provider, error = %e, "Advisor failed");
            }
            Err(e) => {
                warn!(error = %e, "Advisor task panicked");
            }
        }
    }

    if results.is_empty() {
        anyhow::bail!("All advisors failed to produce output");
    }

    Ok(results)
}

/// Query every available provider in parallel.
/// Returns a vec of `(provider, output)` pairs.
pub async fn ask_all(prompt: &str, save: bool) -> Result<Vec<(String, String)>> {
    let providers = available_providers().await;
    ask_providers(&providers, prompt, save, 60).await
}
