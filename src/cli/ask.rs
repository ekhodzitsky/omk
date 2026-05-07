use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};

use crate::runtime::ask::{self, is_known_provider};

/// Ask a provider advisor (cross-consultation)
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Provider: claude, codex, gemini, kimi
    #[arg(value_name = "PROVIDER", default_value = "")]
    pub provider: String,

    /// Question or prompt
    #[arg(trailing_var_arg = true, value_name = "PROMPT")]
    pub prompt: Vec<String>,

    /// Ask all available providers
    #[arg(long)]
    pub all: bool,

    /// Save artifact to .omk/artifacts/
    #[arg(short, long)]
    pub save: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let mut provider = args.provider;
    let mut prompt_parts = args.prompt;

    // If the first positional arg is not a known provider, treat it as part of the prompt.
    if !provider.is_empty() && !is_known_provider(&provider) {
        prompt_parts.insert(0, provider);
        provider = String::new();
    }

    let prompt = prompt_parts.join(" ");
    if prompt.is_empty() {
        anyhow::bail!("Prompt is required");
    }

    let all = args.all || provider.is_empty();

    if all {
        info!("Asking all available providers");
        let outputs = ask::ask_all(&prompt, args.save).await?;

        if ask::is_provider_installed("kimi") {
            info!("Synthesizing with Kimi");
            let synthesis = ask::synthesize(&prompt, &outputs, args.save).await?;
            println!("{}", synthesis);
        } else {
            warn!("Kimi CLI not found; printing individual advisor outputs");
            for (provider_name, output) in &outputs {
                println!("## {}\n\n{}\n", provider_name, output);
            }
        }
    } else {
        info!(provider = %provider, "Asking single provider");
        let output = ask::ask_single(&provider, &prompt, args.save).await?;
        println!("{}", output);
    }

    Ok(())
}
