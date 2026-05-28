use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::llm::planner::{LlmPlanner, Planner};
use crate::llm::types::TokenBudget;
use crate::llm::{LlmClientConfig, WireLlmClient};
use crate::wire::client::ProcessWireClient;

pub(crate) async fn cmd_run(
    goal: &str,
    options: crate::runtime::goal::CreateGoalOptions,
    no_llm_planner: bool,
    planner_token_budget: u32,
) -> Result<()> {
    if options.until_ready {
        // NOTE: run_goal_until_ready hard-codes planner=None internally.
        // Wiring the LLM planner for the --until-ready path requires
        // architectural changes to runtime::goal (out of scope for WS-10).
        let project_dir = std::env::current_dir()
            .context("Failed to resolve current directory for the goal controller loop")?;
        let outcome =
            crate::runtime::goal::run_goal_until_ready(goal, options, &project_dir).await?;
        print_until_ready_outcome(&outcome);
        return Ok(());
    }

    let (planner_holder, disclosure) = if no_llm_planner {
        (None, format_planner_disclosure(PlannerState::Stub))
    } else {
        match build_llm_planner(planner_token_budget).await {
            Ok(p) => (
                Some(Box::new(p) as Box<dyn Planner>),
                format_planner_disclosure(PlannerState::Llm),
            ),
            Err(e) => (
                None,
                format_planner_disclosure(PlannerState::Fallback(e.to_string())),
            ),
        }
    };
    eprintln!("{disclosure}");

    let planner_ref = planner_holder.as_deref();
    let goals_dir = crate::runtime::config::omk_state_dir().join(crate::runtime::goal::GOALS_DIR);
    let existing_entries: std::collections::HashSet<_> =
        match tokio::fs::read_dir(&goals_dir).await {
            Ok(mut rd) => {
                let mut set = std::collections::HashSet::new();
                while let Ok(Some(entry)) = rd.next_entry().await {
                    set.insert(entry.path());
                }
                set
            }
            Err(_) => std::collections::HashSet::new(),
        };

    let state = match crate::runtime::goal::create_goal(goal, options.clone(), planner_ref).await {
        Ok(s) => s,
        Err(e) if planner_ref.is_some() => {
            // Remove any empty goal directories left behind by the failed
            // LLM attempt so that retrying with the stub does not create
            // phantom goals.
            if let Ok(mut new_dirs) = tokio::fs::read_dir(&goals_dir).await {
                while let Ok(Some(entry)) = new_dirs.next_entry().await {
                    let path = entry.path();
                    if !existing_entries.contains(&path) && is_empty_goal_dir(&path).await {
                        if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                            tracing::warn!(path = %path.display(), error = %e, "Failed to remove phantom goal directory");
                        }
                    }
                }
            }
            let reason = format!("LLM planner failed at runtime: {e}");
            eprintln!(
                "{}",
                format_planner_disclosure(PlannerState::Fallback(reason))
            );
            crate::runtime::goal::create_goal(goal, options, None).await?
        }
        Err(e) => return Err(e),
    };
    print_goal_scaffold(&state);
    Ok(())
}

/// Which planner variant is active.
#[derive(Debug, Clone)]
pub(crate) enum PlannerState {
    Llm,
    Stub,
    Fallback(String),
}

/// Format the single-line disclosure message printed to stderr.
pub(crate) fn format_planner_disclosure(state: PlannerState) -> String {
    match state {
        PlannerState::Llm => "goal: using llm planner (kimi)".into(),
        PlannerState::Stub => "goal: using stub planner (--no-llm-planner)".into(),
        PlannerState::Fallback(reason) => format!(
            "goal: llm planner unavailable ({}); falling back to stub planner",
            reason
        ),
    }
}

/// Returns true if the directory exists and has no entries.  Used to clean
/// up goal directories left behind by a failed planner attempt before any
/// files were written.
async fn is_empty_goal_dir(path: &std::path::Path) -> bool {
    match tokio::fs::try_exists(path).await {
        Ok(true) => {}
        _ => return false,
    }
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_dir() => {}
        _ => return false,
    }
    match tokio::fs::read_dir(path).await {
        Ok(mut rd) => matches!(rd.next_entry().await, Ok(None)),
        Err(_) => false,
    }
}

async fn build_llm_planner(
    token_budget: u32,
) -> anyhow::Result<LlmPlanner<WireLlmClient<ProcessWireClient>>> {
    let kimi_bin = which::which("kimi")
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "kimi".to_string());

    let wire = ProcessWireClient::spawn(&kimi_bin, None, None, None)
        .await
        .map_err(|e| anyhow::anyhow!("failed to spawn kimi wire client: {e}"))?;

    let wire_arc = Arc::new(Mutex::new(wire));

    let config = LlmClientConfig {
        model: "kimi-k2".to_string(),
        max_tokens: token_budget as usize,
        temperature: 0.2,
        timeout: std::time::Duration::from_secs(60),
        retry_policy: crate::llm::RetryPolicy::default(),
    };

    let client = WireLlmClient::new(wire_arc, config, crate::llm::CostEstimator::new());
    let budget = TokenBudget::new(token_budget as usize);
    Ok(LlmPlanner::new(Arc::new(client), budget))
}

fn print_goal_scaffold(state: &crate::runtime::goal::GoalState) {
    println!("Goal scaffold created: {}", state.goal_id);
    println!("  Status: {}", state.status);
    println!("  Phase:  {}", state.phase);
    println!("  State:  {}", state.state_dir.display());
    println!(
        "  Proof:  {}",
        state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
    if state.status == crate::runtime::goal::GoalStatus::BlockedOnHuman {
        if let Some(failure) = &state.failure {
            println!();
            println!("Decision needed: {}", failure.reason);
        }
        println!();
        println!("Next: refine the goal with testable success criteria, then run it again.");
        println!("  Example:");
        println!("    omk goal run \"Fix all failing cargo tests in src/runtime/goal\"");
    } else {
        println!();
        println!("Next steps:");
        println!("  1. Inspect the scaffold:  omk goal show latest");
        println!("  2. Run verification:      omk goal verify latest");
        println!("  3. Execute agent wave:    omk goal execute latest");
        println!("  4. Attach reviews:        omk goal review latest");
    }
}

fn print_until_ready_outcome(outcome: &crate::runtime::goal::GoalRunUntilReadyOutcome) {
    println!("Goal run completed: {}", outcome.state.goal_id);
    println!("  Status: {}", outcome.proof.status);
    println!("  Phase:  {}", outcome.state.phase);
    println!("  State:  {}", outcome.state.state_dir.display());
    println!(
        "  Proof:  {}",
        outcome
            .state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
    println!();
    println!("Narrative:");
    for (idx, step) in outcome.steps.iter().enumerate() {
        let icon = step_icon(step.kind);
        println!("  {idx}. {icon} {} — {}", step.kind.as_str(), step.summary);
    }
    if let Some(blocker) = &outcome.blocker {
        println!();
        if outcome.state.status == crate::runtime::goal::GoalStatus::BlockedOnHuman {
            println!("Decision needed: {blocker}");
            println!("Next: refine the goal with testable success criteria, then run it again.");
        } else {
            println!("Blocked: {blocker}");
            println!("GitHub mutation: disabled");
            println!("Merge policy: manual");
            if let Some(path) = &outcome.policy_evidence_path {
                println!("Policy evidence: {}", path.display());
            }
        }
    }
}

fn step_icon(kind: crate::runtime::goal::GoalControllerStepKind) -> &'static str {
    use crate::runtime::goal::GoalControllerStepKind;
    match kind {
        GoalControllerStepKind::Plan => "📋",
        GoalControllerStepKind::Verify => "🔍",
        GoalControllerStepKind::Execute => "⚡",
        GoalControllerStepKind::Review => "👁 ",
        GoalControllerStepKind::Deliver => "🚀",
        GoalControllerStepKind::Blocked => "🚧",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disclosure_format_llm() {
        assert_eq!(
            format_planner_disclosure(PlannerState::Llm),
            "goal: using llm planner (kimi)"
        );
    }

    #[test]
    fn test_disclosure_format_stub() {
        assert_eq!(
            format_planner_disclosure(PlannerState::Stub),
            "goal: using stub planner (--no-llm-planner)"
        );
    }

    #[test]
    fn test_disclosure_format_fallback() {
        let msg = format_planner_disclosure(PlannerState::Fallback("no binary".into()));
        assert_eq!(
            msg,
            "goal: llm planner unavailable (no binary); falling back to stub planner"
        );
    }
}
