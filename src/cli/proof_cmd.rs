use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: ProofCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ProofCommands {
    /// Generate or show a proof for a run
    Show {
        /// Run ID or "latest"
        run_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "md")]
        format: OutputFormat,
        /// Regenerate proof from events even if a cached proof exists
        #[arg(long)]
        regenerate: bool,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
    Md,
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        ProofCommands::Show {
            run_id,
            format,
            regenerate,
        } => cmd_show(&run_id, format, regenerate).await,
    }
}

async fn cmd_show(run_id: &str, format: OutputFormat, regenerate: bool) -> Result<()> {
    let (state_dir, resolved_run_id) = crate::runtime::state::resolve_run(run_id).await?;
    let event_log = state_dir.join(crate::runtime::config::EVENTS_FILE);

    let proof = if regenerate || !crate::runtime::proof::Proof::proof_path(&state_dir).exists() {
        if !event_log.exists() {
            anyhow::bail!("No events found for run '{}'", resolved_run_id);
        }
        let proof = crate::runtime::proof::ProofGenerator::from_events(
            &crate::runtime::events::RunId(resolved_run_id.clone()),
            &event_log,
        )
        .await?;
        proof.save(&state_dir).await?;
        proof
    } else {
        crate::runtime::proof::Proof::load(&state_dir)
            .await?
            .unwrap_or_else(|| {
                println!("No cached proof found, generating from events...");
                // This branch shouldn't normally be reached due to the check above
                crate::runtime::proof::Proof::new(crate::runtime::events::RunId(
                    resolved_run_id.clone(),
                ))
            })
    };

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&proof)?);
        }
        OutputFormat::Md => {
            println!("{}", proof.to_markdown());
        }
        OutputFormat::Text => {
            let (gates_passed, gates_failed, gates_skipped) = proof.gate_counts();
            println!("Proof Report for {}", proof.run_id);
            println!("{}", "=".repeat(60));
            println!("Status:      {}", proof.status);
            println!("Readiness:   {}", proof.readiness());
            println!("Generated:   {}", proof.generated_at);
            if proof.elapsed_secs > 0 {
                println!("Duration:    {}s", proof.elapsed_secs);
            }
            println!();

            println!("Verdict:");
            println!("  status:          {}", proof.status);
            println!("  readiness:       {}", proof.readiness());
            println!("  changed_files:   {}", proof.changed_files.len());
            println!("  gates_total:     {}", proof.gates.len());
            println!(
                "  gates:           passed={}, failed={}, skipped={}",
                gates_passed, gates_failed, gates_skipped
            );
            println!("  failures:        {}", proof.failures.len());
            println!("  retries:         {}", proof.retries.len());
            println!("  known_gaps:      {}", proof.known_gaps.len());
            println!();

            println!("Changed files ({}):", proof.changed_files.len());
            if proof.changed_files.is_empty() {
                println!("  none");
            } else {
                for f in &proof.changed_files {
                    println!("  {:10} {}", f.operation, f.path);
                }
            }
            println!();

            println!("Gates ({}):", proof.gates.len());
            if proof.gates.is_empty() {
                println!("  none");
            } else {
                for g in &proof.gates {
                    let status_text = match g.status {
                        crate::runtime::proof::GateStatus::Passed => "passed",
                        crate::runtime::proof::GateStatus::Failed => "failed",
                        crate::runtime::proof::GateStatus::Skipped => "skipped",
                    };
                    let required_str = if g.required { "required" } else { "optional" };
                    println!("  {:8} {:20} {}", status_text, g.name, required_str);
                }
            }
            println!();

            println!("Failures ({}):", proof.failures.len());
            if proof.failures.is_empty() {
                println!("  none");
            } else {
                for f in &proof.failures {
                    println!(
                        "  - {}: {}",
                        f.worker_id.as_deref().unwrap_or("?"),
                        f.description
                    );
                }
            }
            println!();

            println!("Retries ({}):", proof.retries.len());
            if proof.retries.is_empty() {
                println!("  none");
            } else {
                for r in &proof.retries {
                    println!("  - {} (attempt {}): {}", r.task_id, r.attempt, r.reason);
                }
            }
            println!();

            println!("Known gaps ({}):", proof.known_gaps.len());
            if proof.known_gaps.is_empty() {
                println!("  none");
            } else {
                for g in &proof.known_gaps {
                    println!("  - {}", g);
                }
            }
            println!();

            println!("Summary: {}", proof.summary);
            println!();
            println!("{}", "=".repeat(60));

            println!("Readiness verdict: {}.", proof.readiness());
        }
    }

    Ok(())
}
