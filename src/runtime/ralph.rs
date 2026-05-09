// Ralph persistent loop — prd.json + verify/fix
#![allow(dead_code)] // API surface for future features (verify_story, story escalation)
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::runtime::gates::{
    detect_changed_files, format_gate_summary, gates_passed, load_or_detect_gates, run_gates,
    DoneContract,
};
use crate::runtime::state::{Prd, RalphState, StoryStatus, UserStory};

static WORD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\w+\b").unwrap());

fn slugify_task(task: &str) -> String {
    let words: Vec<&str> = WORD_RE.find_iter(task).map(|m| m.as_str()).collect();
    let slug = words[..words.len().min(5)].join("-").to_lowercase();
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

/// Generate a PRD by breaking a task into 3–5 user stories.
pub fn generate_prd(task: &str) -> Prd {
    let sentences: Vec<&str> = task
        .split(['.', '?', '!'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let chunks: Vec<String> = if sentences.len() >= 3 {
        sentences.into_iter().map(String::from).collect()
    } else {
        task.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let stories: Vec<UserStory> = chunks
        .into_iter()
        .enumerate()
        .map(|(i, desc)| UserStory {
            id: format!("US-{:03}", i + 1),
            description: desc.clone(),
            acceptance_criteria: vec![
                format!("{} is implemented correctly", desc),
                "All related tests pass".to_string(),
            ],
            status: StoryStatus::NotStarted,
        })
        .take(5)
        .collect();

    if stories.is_empty() {
        Prd {
            user_stories: vec![UserStory {
                id: "US-001".to_string(),
                description: task.to_string(),
                acceptance_criteria: vec![
                    format!("{} is implemented correctly", task),
                    "All related tests pass".to_string(),
                ],
                status: StoryStatus::NotStarted,
            }],
        }
    } else {
        Prd {
            user_stories: stories,
        }
    }
}

/// Spawn `kimi -p` and capture its combined output.
pub async fn run_kimi(prompt: &str, dir: &Path) -> Result<String> {
    let output = Command::new("kimi")
        .args(["-p", prompt])
        .current_dir(dir)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        warn!(status = ?output.status, stderr = %stderr, "kimi command failed");
    }

    Ok(format!("{}{}", stdout, stderr))
}

/// Run `cargo test` in the given directory and return whether it succeeded.
pub async fn run_tests(dir: &Path) -> Result<bool> {
    let output = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(dir)
        .output()
        .await?;

    Ok(output.status.success())
}

/// MVP verification: story passes if the test suite passes.
fn verify_story(_story: &UserStory, _kimi_output: &str, tests_pass: bool) -> bool {
    tests_pass
}

/// Compute the Ralph state directory for a task.
pub fn state_dir_for(_dir: &Path, task: &str) -> PathBuf {
    crate::runtime::config::state_dir()
        .join("ralph")
        .join(slugify_task(task))
}

/// Run the Ralph persistent loop.
pub async fn run_ralph(
    task: &str,
    dir: &Path,
    max_iterations: usize,
    resume: bool,
    yolo: bool,
) -> Result<()> {
    info!(task = %task, dir = %dir.display(), max_iterations, resume, yolo, "Starting Ralph persistent loop");

    let started_at = chrono::Utc::now();
    let agents_md = match crate::agents::load_project_agents(dir).await {
        Ok(Some(m)) => Some(m),
        _ => None,
    };
    let gate_config = load_or_detect_gates(dir).await;
    let state_dir = state_dir_for(dir, task);
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

    println!("Ralph: starting persistence loop for '{}'", task);
    println!("  Stories: {}", state.prd.user_stories.len());
    println!("  Max iterations: {}", max_iterations);
    println!("  State dir: {}", state_dir.display());

    // Show rough cost estimate
    let rough_estimate = crate::cost::estimator::estimate_ralph_cost(
        300,
        max_iterations,
        state.prd.user_stories.len(),
    );
    println!("  Estimated cost: {}", rough_estimate.formatted());

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
                println!("✓ All user stories verified. Ralph complete.");
                state.save().await?;

                // Record cost
                let duration = chrono::Utc::now()
                    .signed_duration_since(started_at)
                    .num_seconds()
                    .max(0) as u64;
                let verified = state
                    .prd
                    .user_stories
                    .iter()
                    .filter(|s| matches!(s.status, StoryStatus::Verified))
                    .count();
                let cost = crate::cost::estimator::estimate_ralph_cost(
                    duration,
                    state.iteration,
                    state.prd.user_stories.len(),
                );
                let _ = crate::runtime::session::record_session_end(
                    "ralph",
                    task,
                    started_at,
                    cost,
                    crate::notifications::NotificationEvent::RalphComplete {
                        name: task.to_string(),
                        duration_secs: duration,
                        iterations: state.iteration,
                        verified,
                        total: state.prd.user_stories.len(),
                    },
                )
                .await;

                // Save done contract
                let mut contract = DoneContract::new(
                    &format!("ralph-{}", slugify_task(task)),
                    "ralph",
                    started_at,
                );
                contract.gates = state.gate_results.clone();
                contract.passed = true;
                contract.changed_files = detect_changed_files(dir).await;
                let _ = contract.save(&state_dir.join("done-contract.json")).await;

                return Ok(());
            }
        };

        let story_id = state.prd.user_stories[story_idx].id.clone();
        let story_desc = state.prd.user_stories[story_idx].description.clone();
        let failures = consecutive_failures.get(&story_id).copied().unwrap_or(0);

        println!(
            "[{}/{}] Story {}: {}",
            state.iteration, max_iterations, story_id, story_desc
        );

        if failures >= 3 {
            warn!(story_id = %story_id, "Escalating to architect after 3 failures");
            println!(
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
                    println!("  Architect provided guidance ({} bytes)", output.len());
                }
                Err(e) => {
                    error!(error = %e, "Architect escalation failed");
                    println!("  ⚠ Architect escalation failed: {}", e);
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

        println!("  Verifying {}...", story_id);

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
                    }]
                }
            }
        } else {
            let results = run_gates(&gate_config, dir).await;
            state.gate_results = results.clone();
            println!("{}", format_gate_summary(&results));
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
            println!("  ✓ {} verified", story_id);
        } else {
            state.prd.user_stories[story_idx].status = StoryStatus::Failed;
            let new_failures = failures + 1;
            consecutive_failures.insert(story_id.clone(), new_failures);
            println!("  ✗ {} failed (attempt {}/3)", story_id, new_failures);
            if !yolo && new_failures >= 3 {
                println!("  ⚠ Max failures reached. Use --yolo to continue.");
                state.save().await?;
                // Save done contract before bail
                let mut contract = DoneContract::new(
                    &format!("ralph-{}", slugify_task(task)),
                    "ralph",
                    started_at,
                );
                contract.gates = gate_results;
                contract.passed = false;
                let _ = contract.save(&state_dir.join("done-contract.json")).await;
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

    println!("Ralph: reached max iterations ({})", max_iterations);
    info!("Ralph reached max iterations");
    state.save().await?;

    let duration = chrono::Utc::now()
        .signed_duration_since(started_at)
        .num_seconds()
        .max(0) as u64;
    let verified = state
        .prd
        .user_stories
        .iter()
        .filter(|s| matches!(s.status, StoryStatus::Verified))
        .count();

    // Save done contract
    let mut contract = DoneContract::new(
        &format!("ralph-{}", slugify_task(task)),
        "ralph",
        started_at,
    );
    contract.gates = state.gate_results.clone();
    contract.passed = verified == state.prd.user_stories.len();
    contract.changed_files = detect_changed_files(dir).await;
    let _ = contract.save(&state_dir.join("done-contract.json")).await;

    // Record cost
    let cost = crate::cost::estimator::estimate_ralph_cost(
        duration,
        state.iteration,
        state.prd.user_stories.len(),
    );
    let _ = crate::runtime::session::record_session_end(
        "ralph",
        task,
        started_at,
        cost,
        crate::notifications::NotificationEvent::RalphComplete {
            name: task.to_string(),
            duration_secs: duration,
            iterations: state.iteration,
            verified,
            total: state.prd.user_stories.len(),
        },
    )
    .await;

    Ok(())
}

fn print_progress(state: &RalphState) {
    let verified = state
        .prd
        .user_stories
        .iter()
        .filter(|s| matches!(s.status, StoryStatus::Verified))
        .count();
    let failed = state
        .prd
        .user_stories
        .iter()
        .filter(|s| matches!(s.status, StoryStatus::Failed))
        .count();
    let total = state.prd.user_stories.len();

    println!();
    println!(
        "🔄 Ralph: {}/{} stories verified, {} failed (iteration {}/{})",
        verified, total, failed, state.iteration, state.max_iterations
    );
    for story in &state.prd.user_stories {
        let icon = match story.status {
            StoryStatus::Verified => "✓",
            StoryStatus::Failed => "✗",
            StoryStatus::InProgress => "▶",
            StoryStatus::Implemented => "◐",
            StoryStatus::NotStarted => "○",
        };
        println!("   {} {}", icon, story.id);
    }
    println!();
}
