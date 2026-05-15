use anyhow::Result;

use super::artifact::save_artifact;
use super::execution::run_advisor_direct;
use super::provider::is_provider_installed;

/// Build the synthesis prompt fed to Kimi.
pub fn build_synthesis_prompt(prompt: &str, outputs: &[(String, String)]) -> String {
    let mut synthesis = format!(
        "The user asked: {}\n\nHere are answers from multiple AI advisors:\n\n",
        prompt
    );
    for (provider, output) in outputs {
        synthesis.push_str(&format!("## {}\n\n{}\n\n", provider, output));
    }
    synthesis.push_str(
        "Please synthesize a unified answer that combines the best insights from all advisors. \
         Be concise but thorough.\n",
    );
    synthesis
}

/// Synthesize multiple advisor outputs into a unified answer using Kimi.
pub async fn synthesize(prompt: &str, outputs: &[(String, String)], save: bool) -> Result<String> {
    if !is_provider_installed("kimi").await {
        anyhow::bail!("Kimi CLI is required for synthesis but is not installed");
    }

    let synthesis_prompt = build_synthesis_prompt(prompt, outputs);
    let result = run_advisor_direct("kimi", &synthesis_prompt, 120).await?;

    if save {
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let content = format!("# Synthesis\n\nOriginal prompt: {prompt}\n\n---\n\n{result}\n");
        save_artifact("synthesis", &content, &ts).await?;
    }

    Ok(result)
}
