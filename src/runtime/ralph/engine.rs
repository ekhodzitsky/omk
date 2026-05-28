// Ralph persistent loop — prd.json + verify/fix
#![allow(dead_code)] // API surface for future features (verify_story, story escalation)
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{error, info, warn};

use crate::runtime::gates::{
    detect_changed_files, format_gate_summary, gates_passed, load_or_detect_gates, run_gates,
    DoneContract,
};
use crate::runtime::state::{RalphState, StoryStatus, UserStory};

use super::generate::{generate_prd, slugify_task};
use super::progress::print_progress;
use super::runner::{run_kimi, run_tests};

/// Compute the Ralph state directory for a task.
pub fn state_dir_for(_dir: &Path, task: &str) -> Result<PathBuf> {
    Ok(crate::runtime::config::state_dir()
        .join("ralph")
        .join(slugify_task(task)?))
}

/// MVP verification: story passes if the test suite passes.
fn verify_story(_story: &UserStory, _kimi_output: &str, tests_pass: bool) -> bool {
    tests_pass
}

/// Run the Ralph persistent loop.
pub async fn run_ralph(
    task: &str,
    dir: &Path,
    max_iterations: usize,
    resume: bool,
    yolo: bool,
) -> Result<crate::runtime::session::SessionSummary> {
    info!(task = %task, dir = %dir.display(), max_iterations, resume, yolo, "Starting Ralph persistent loop");

    let started_at = chrono::Utc::now();
    let agents_md = match crate::agents::load_project_agents(dir).await {
        Ok(Some(m)) => Some(m),
        _ => None,
    };
    let gate_config = load_or_detect_gates(dir).await;
    let state_dir = state_dir_for(dir, task)?;
    tokio::fs::create_dir_all(&state_dir).await?;

    let mut state = if resume {
        match RalphState::load(&state_dir).await {
            Ok(mut existing) => {
                info!(
                    iteration = existing.iteration,
                    "Resumed existing Ralph state"
                );
                existing.max_iterations = max_iterations;
                existing
            }
            Err(_) => {
                anyhow::bail!(
                    "No existing Ralph state found for '{}' at {}",
                    task,
                    state_dir.display()
                );
            }
        }
    } else {
        match RalphState::load(&state_dir).await {
            Ok(mut existing) => {
                info!(
                    iteration = existing.iteration,
                    "Resumed existing Ralph state"
                );
                existing.max_iterations = max_iterations;
                existing
            }
            Err(_) => {
                let prd = generate_prd(task);
                info!(stories = prd.user_stories.len(), "Generated PRD");
                RalphState {
                    version: 1,
                    task: task.to_string(),
                    prd,
                    iteration: 0,
                    max_iterations,
                    state_dir: state_dir.clone(),
                    gate_results: vec![],
                }
            }
        }
    };

    let prd_path = state_dir.join("prd.json");
    tokio::fs::write(&prd_path, serde_json::to_string_pretty(&state.prd)?).await?;
    info!(path = %prd_path.display(), "Saved PRD");

    info!("Ralph: starting persistence loop for '{}'", task);
    info!("  Stories: {}", state.prd.user_stories.len());
    info!("  Max iterations: {}", max_iterations);
    info!("  State dir: {}", state_dir.display());

    // Show rough cost estimate
    let rough_estimate = crate::cost::estimator::estimate_ralph_cost(
        300,
        max_iterations,
        state.prd.user_stories.len(),
    );
    info!("  Estimated cost: {}", rough_estimate.formatted());

    let mut consecutive_failures: HashMap<String, usize> = HashMap::new();

    print_progress(&state);

    while state.iteration < state.max_iterations {
        state.iteration += 1;
        info!(iteration = state.iteration, "Ralph iteration start");

        let story_idx = match state.prd.user_stories.iter().position(|s| {
            matches!(
                s.status,
                StoryStatus::NotStarted | StoryStatus::InProgress | StoryStatus::Failed
            )
        }) {
            Some(idx) => idx,
            None => {
                info!("All stories verified — Ralph loop complete");
                info!("✓ All user stories verified. Ralph complete.");
                state.save().await?;

                let duration = u64::try_from(
                    chrono::Utc::now()
                        .signed_duration_since(started_at)
                        .num_seconds(),
                )
                .unwrap_or(0);
                let verified = state
                    .prd
                    .user_stories
                    .iter()
                    .filter(|s| matches!(s.status, StoryStatus::Verified))
                    .count();

                // Save done contract
                let mut contract = DoneContract::new(
                    &format!("ralph-{}", slugify_task(task)?),
                    "ralph",
                    started_at,
                );
                contract.gates = state.gate_results.clone();
                contract.passed = true;
                contract.changed_files = detect_changed_files(dir).await;
                contract.save(&state_dir.join("done-contract.json")).await?;

                return Ok(crate::runtime::session::SessionSummary {
                    session_type: "ralph".to_string(),
                    name: task.to_string(),
                    started_at,
                    ended_at: chrono::Utc::now(),
                    duration_secs: duration,
                    jobs_total: None,
                    jobs_success: None,
                    phases_completed: None,
                    iterations: Some(state.iteration),
                    verified: Some(verified),
                    total_stories: Some(state.prd.user_stories.len()),
                });
            }
        };

        let story_id = state.prd.user_stories[story_idx].id.clone();
        let story_desc = state.prd.user_stories[story_idx].description.clone();
        let failures = consecutive_failures.get(&story_id).copied().unwrap_or(0);

        info!(
            "[{}/{}] Story {}: {}",
            state.iteration, max_iterations, story_id, story_desc
        );

        if failures >= 3 {
            warn!(story_id = %story_id, "Escalating to architect after 3 failures");
            info!(
                "  ⚠ Escalating {} to architect (3 failed attempts)",
                story_id
            );

            let base_escalation = format!(
                "Architect review needed for story {}: {}. \
                Previous implementation attempts failed {} times. \
                Provide a detailed implementation plan.",
                story_id, story_desc, failures
            );
            let escalation_prompt = if let Some(ref manifest) = agents_md {
                format!(
                    "{}\n\n{}",
                    base_escalation,
                    crate::agents::inject_agents_context(manifest, task, "architect")
                )
            } else {
                base_escalation
            };

            match run_kimi(&escalation_prompt, dir).await {
                Ok(output) => {
                    info!(
                        output_len = output.len(),
                        "Architect escalation response received"
                    );
                    info!("  Architect provided guidance ({} bytes)", output.len());
                }
                Err(e) => {
                    error!(error = %e, "Architect escalation failed");
                    warn!("  ⚠ Architect escalation failed: {}", e);
                }
            }

            consecutive_failures.insert(story_id.clone(), 0);
        }

        state.prd.user_stories[story_idx].status = StoryStatus::InProgress;
        state.save().await?;

        let base_impl = format!(
            "Implement the following user story precisely. \
            Make minimal, focused changes. Run tests after implementing.\n\n\
            Story ID: {}\nDescription: {}\nAcceptance Criteria:\n- {}\n\n\
            Output a summary of changes made.",
            story_id,
            story_desc,
            state.prd.user_stories[story_idx]
                .acceptance_criteria
                .join("\n- ")
        );
        let impl_prompt = if let Some(ref manifest) = agents_md {
            format!(
                "{}\n\n{}",
                base_impl,
                crate::agents::inject_agents_context(manifest, task, "implementer")
            )
        } else {
            base_impl
        };

        let _kimi_output = match run_kimi(&impl_prompt, dir).await {
            Ok(output) => {
                info!(
                    output_len = output.len(),
                    "Implementation response received"
                );
                output
            }
            Err(e) => {
                warn!(error = %e, "Failed to spawn kimi for implementation");
                format!("Error: {}", e)
            }
        };

        state.prd.user_stories[story_idx].status = StoryStatus::Implemented;
        state.save().await?;

        info!("  Verifying {}...", story_id);

        let gate_results = if gate_config.gates.is_empty() {
            // No gates configured — fall back to old behavior (just tests)
            match run_tests(dir).await {
                Ok(true) => vec![],
                _ => {
                    vec![crate::runtime::gates::GateResult {
                        name: "tests".to_string(),
                        passed: false,
                        stdout: String::new(),
                        stderr: "No gates configured and tests failed".to_string(),
                        duration_ms: 0,
                        required: true,
                        command_line: "cargo test".to_string(),
                        exit_code: Some(1),
                        timed_out: false,
                        stdout_summary: None,
                        stderr_summary: Some("No gates configured and tests failed".to_string()),
                        output_path: None,
                        timeout_secs: 0,
                        circuit_breaker_open: false,
                    }]
                }
            }
        } else {
            let results = run_gates(&gate_config, dir).await;
            state.gate_results = results.clone();
            info!("{}", format_gate_summary(&results));
            results
        };

        let passed = if gate_config.gates.is_empty() {
            matches!(run_tests(dir).await, Ok(true))
        } else {
            gates_passed(&gate_results)
        };

        if passed {
            state.prd.user_stories[story_idx].status = StoryStatus::Verified;
            consecutive_failures.insert(story_id.clone(), 0);
            info!("  ✓ {} verified", story_id);
        } else {
            state.prd.user_stories[story_idx].status = StoryStatus::Failed;
            let new_failures = failures + 1;
            consecutive_failures.insert(story_id.clone(), new_failures);
            warn!("  ✗ {} failed (attempt {}/3)", story_id, new_failures);
            if !yolo && new_failures >= 3 {
                warn!("  ⚠ Max failures reached. Use --yolo to continue.");
                state.save().await?;
                // Save done contract before bail
                let mut contract = DoneContract::new(
                    &format!("ralph-{}", slugify_task(task)?),
                    "ralph",
                    started_at,
                );
                contract.gates = gate_results;
                contract.passed = false;
                contract.changed_files = detect_changed_files(dir).await;
                contract.save(&state_dir.join("done-contract.json")).await?;
                anyhow::bail!("Story {} failed too many times", story_id);
            }
        }

        state.save().await?;
        print_progress(&state);
        info!(
            iteration = state.iteration,
            story_id = %story_id,
            status = ?state.prd.user_stories[story_idx].status,
            "Ralph iteration complete"
        );
    }

    info!("Ralph: reached max iterations ({})", max_iterations);
    info!("Ralph reached max iterations");
    state.save().await?;

    let duration = u64::try_from(
        chrono::Utc::now()
            .signed_duration_since(started_at)
            .num_seconds(),
    )
    .unwrap_or(0);
    let verified = state
        .prd
        .user_stories
        .iter()
        .filter(|s| matches!(s.status, StoryStatus::Verified))
        .count();

    // Save done contract
    let mut contract = DoneContract::new(
        &format!("ralph-{}", slugify_task(task)?),
        "ralph",
        started_at,
    );
    contract.gates = state.gate_results.clone();
    contract.passed = verified == state.prd.user_stories.len();
    contract.changed_files = detect_changed_files(dir).await;
    contract.save(&state_dir.join("done-contract.json")).await?;

    Ok(crate::runtime::session::SessionSummary {
        session_type: "ralph".to_string(),
        name: task.to_string(),
        started_at,
        ended_at: chrono::Utc::now(),
        duration_secs: duration,
        jobs_total: None,
        jobs_success: None,
        phases_completed: None,
        iterations: Some(state.iteration),
        verified: Some(verified),
        total_stories: Some(state.prd.user_stories.len()),
    })
}
