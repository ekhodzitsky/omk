use anyhow::Result;
use clap::Parser;
use tracing::info;

use crate::runtime::ask::{self, is_known_provider};

/// Ask a provider advisor (cross-consultation)
#[derive(Parser, Debug, Clone)]
pub(crate) struct Args {
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

    /// Disable synthesis (print individual outputs)
    #[arg(long)]
    pub no_synthesis: bool,

    /// Specific providers to ask (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub providers: Vec<String>,

    /// Timeout per provider in seconds
    #[arg(short, long, default_value = "60")]
    pub timeout: u64,
}

pub(crate) async fn run(args: Args) -> Result<()> {
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

    if all || !args.providers.is_empty() {
        let providers = if args.providers.is_empty() {
            ask::available_providers().await
        } else {
            args.providers.iter().map(|s| s.as_str()).collect()
        };

        if providers.is_empty() {
            anyhow::bail!("No provider CLIs are installed");
        }

        info!(providers = ?providers, "Asking providers");
        let outputs = ask::ask_providers(&providers, &prompt, args.save, args.timeout).await?;

        if !args.no_synthesis && ask::is_provider_installed("kimi").await && outputs.len() > 1 {
            info!("Synthesizing with Kimi");
            let synthesis = ask::synthesize(&prompt, &outputs, args.save).await?;
            println!("{}", synthesis);
        } else {
            if args.no_synthesis {
                info!("Synthesis disabled");
            }
            for (provider_name, output) in &outputs {
                println!("## {}\n\n{}\n", provider_name, output);
            }
        }
    } else {
        info!(provider = %provider, "Asking single provider");
        let output = ask::ask_single(&provider, &prompt, args.save, args.timeout).await?;
        println!("{}", output);
    }

    Ok(())
}
