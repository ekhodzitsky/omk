use anyhow::Result;
use clap::Parser;

/// Ask a provider advisor (cross-consultation)
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Provider: claude, codex, gemini, kimi
    #[arg(value_name = "PROVIDER")]
    pub provider: String,

    /// Question or prompt
    #[arg(trailing_var_arg = true, value_name = "PROMPT")]
    pub prompt: Vec<String>,

    /// Save artifact to .omk/artifacts/
    #[arg(short, long)]
    pub save: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let prompt = args.prompt.join(" ");
    if prompt.is_empty() {
        anyhow::bail!("Prompt is required");
    }

    println!("omk ask {}: {}", args.provider, prompt);
    println!("(not yet implemented — will spawn {} in tmux and save artifact)", args.provider);

    Ok(())
}
