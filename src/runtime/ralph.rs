// Ralph persistent loop — prd.json + verify/fix
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::process::Command;
use tracing::{error, info, warn};

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
        .split(|c| c == '.' || c == '?' || c == '!')
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
        Prd { user_stories: stories }
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
pub fn state_dir_for(dir: &Path, task: &str) -> PathBuf {
    dir.join(".omk")
        .join("state")
        .join("ralph")
        .join(slugify_task(task))
}

/// Run the Ralph persistent loop.
pub async fn run_ralph(task: &str, dir: &Path, max_iterations: usize) -> Result<()> {
    info!(task = %task, dir = %dir.display(), max_iterations, "Starting Ralph persistent loop");

    let state_dir = state_dir_for(dir, task);
    tokio::fs::create_dir_all(&state_dir).await?;

    let mut state = match RalphState::load(&state_dir).await {
        Ok(mut existing) => {
            info!(iteration = existing.iteration, "Resumed existing Ralph state");
            existing.max_iterations = max_iterations;
            existing
        }
        Err(_) => {
            let prd = generate_prd(task);
            info!(stories = prd.user_stories.len(), "Generated PRD");
            RalphState {
                task: task.to_string(),
                prd,
                iteration: 0,
                max_iterations,
                state_dir: state_dir.clone(),
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

    let mut consecutive_failures: HashMap<String, usize> = HashMap::new();

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
            println!("  ⚠ Escalating {} to architect (3 failed attempts)", story_id);

            let escalation_prompt = format!(
                "Architect review needed for story {}: {}. \
                Previous implementation attempts failed {} times. \
                Provide a detailed implementation plan.",
                story_id, story_desc, failures
            );

            match run_kimi(&escalation_prompt, dir).await {
                Ok(output) => {
                    info!(output_len = output.len(), "Architect escalation response received");
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

        let impl_prompt = format!(
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

        let kimi_output = match run_kimi(&impl_prompt, dir).await {
            Ok(output) => {
                info!(output_len = output.len(), "Implementation response received");
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
        let tests_pass = match run_tests(dir).await {
            Ok(pass) => {
                info!(tests_pass = pass, "Test verification complete");
                pass
            }
            Err(e) => {
                warn!(error = %e, "Test command failed");
                false
            }
        };

        let passed = verify_story(&state.prd.user_stories[story_idx], &kimi_output, tests_pass);

        if passed {
            state.prd.user_stories[story_idx].status = StoryStatus::Verified;
            consecutive_failures.insert(story_id.clone(), 0);
            println!("  ✓ {} verified", story_id);
        } else {
            state.prd.user_stories[story_idx].status = StoryStatus::Failed;
            let new_failures = failures + 1;
            consecutive_failures.insert(story_id.clone(), new_failures);
            println!("  ✗ {} failed (attempt {}/3)", story_id, new_failures);
        }

        state.save().await?;
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
    Ok(())
}
