use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

use super::super::state::{
    GoalState, GOAL_PRD_FILE, GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
use super::super::task_graph::GoalTaskGraph;

const ORACLE_ARTIFACTS_DIR: &str = "artifacts/oracles";
const GREENFIELD_ACCEPTANCE_FILE: &str = "artifacts/oracles/greenfield-acceptance.md";
const GREENFIELD_DEMO_FILE: &str = "artifacts/oracles/greenfield-demo.sh";
const GREENFIELD_USAGE_FILE: &str = "artifacts/oracles/usage-examples.md";

pub(super) async fn write_goal_brief(state: &GoalState, generated_at: DateTime<Utc>) -> Result<()> {
    let body = format!(
        "# Goal Brief\n\n\
         Generated: {generated_at}\n\n\
         ## Original Goal\n\n\
         {}\n\n\
         ## Normalized Goal\n\n\
         {}\n\n\
         ## Current Scope\n\n\
         This scaffold captures intent and durable planning artifacts. Run `omk goal execute` to attach local gate evidence and a bounded Wire-backed agent wave, then `omk goal review` to attach controller review/security evidence.\n",
        state.original_goal, state.normalized_goal
    );
    crate::runtime::atomic::atomic_write(&state.state_dir.join(GOAL_PRD_FILE), body.as_bytes())
        .await
}

pub(super) async fn write_technical_plan(
    state: &GoalState,
    generated_at: DateTime<Utc>,
) -> Result<()> {
    let body = format!(
        "# Technical Plan\n\n\
         Generated: {generated_at}\n\n\
         ## Controller Phases\n\n\
         1. Intake records the original user goal.\n\
         2. Planning writes this technical plan and the goal brief.\n\
         3. Decomposition writes a task graph.\n\
         4. Verification design writes a test specification.\n\
         5. Local execution runs verification gates and refreshes proof evidence.\n\
         6. Review records controller review and security evidence.\n\
         7. Proof writes an honest not-ready proof until required execution, review, and integration evidence exists.\n\n\
         ## Execution Boundary\n\n\
         The current controller records planning task evidence, local verification task evidence, a bounded Wire-backed `goal-agent-execute` wave, agent mutation diff/changed-file evidence, post-mutation gate reruns when files change, and controller review/security evidence. The initial wave can make minimal project changes under the worker boundary, but readiness still requires specialist review loops and integration acceptance.\n\n\
         Goal ID: `{}`\n",
        state.goal_id
    );
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(GOAL_TECHNICAL_PLAN_FILE),
        body.as_bytes(),
    )
    .await
}

pub(super) async fn write_test_spec(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    project_dir: &Path,
    generated_at: DateTime<Utc>,
) -> Result<()> {
    let task_lines = task_graph
        .tasks
        .iter()
        .map(|task| format!("- `{}`: {}", task.id, task.acceptance.join("; ")))
        .collect::<Vec<_>>()
        .join("\n");
    let oracle = super::super::oracle::assess_goal_oracle_evidence(&state.normalized_goal, &[]);
    let oracle_lines = oracle
        .checks
        .iter()
        .map(|check| format!("- `{}`", check.name))
        .collect::<Vec<_>>()
        .join("\n");
    let compatibility_plan = compatibility_plan(&oracle, project_dir).await?;
    let body = format!(
        "# Test Spec\n\n\
         Generated: {generated_at}\n\n\
         ## Required Proof Before Ready\n\n\
         - Required gates must pass.\n\
         - A proof artifact must cite gate evidence.\n\
         - Known gaps must be empty or explicitly accepted by a human.\n\n\
         ## Oracle Criteria\n\n\
         Oracle kind: `{}`\n\n\
         Required checks before `ready`:\n\n\
         {oracle_lines}\n\n\
         ## Readiness Levels\n\n\
         - Engineering-ready: local gates, execution, review, oracle, and integration evidence support handoff.\n\
         - Product-ready: a human release owner has accepted UX, documentation, rollout, and user-facing quality beyond local engineering proof.\n\n\
         {compatibility_plan}\
         ## Scaffold Task Acceptance\n\n\
         {task_lines}\n\n\
         ## Current Status\n\n\
         `omk goal` remains `not_ready` because readiness still requires integration acceptance and specialist review loops beyond controller-owned planning, local verification, bounded agent execution, mutation evidence, post-mutation gate reruns, and controller review/security evidence.\n\n\
         Goal ID: `{}`\n",
        oracle.kind.as_str(),
        state.goal_id
    );
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(GOAL_TEST_SPEC_FILE),
        body.as_bytes(),
    )
    .await
}

pub(super) async fn write_greenfield_oracle_artifacts(
    state: &GoalState,
) -> Result<Vec<(&'static str, &'static str)>> {
    let kind = super::super::oracle::classify_goal_kind(&state.normalized_goal);
    if kind != super::super::oracle::GoalKind::Greenfield {
        return Ok(Vec::new());
    }

    tokio::fs::create_dir_all(state.state_dir.join(ORACLE_ARTIFACTS_DIR)).await?;
    write_relative(
        state,
        GREENFIELD_ACCEPTANCE_FILE,
        &format!(
            "# Greenfield Acceptance Oracle\n\n\
             Goal: {}\n\n\
             Required checks before `ready`:\n\n\
             - acceptance: user-visible behavior satisfies the goal brief.\n\
             - smoke: primary command or UI path starts without errors.\n\
             - demo: a minimal end-to-end usage path can be shown locally.\n",
            state.normalized_goal
        ),
    )
    .await?;
    write_relative(
        state,
        GREENFIELD_DEMO_FILE,
        "#!/bin/sh\nset -eu\nprintf 'Run the greenfield demo path and capture output evidence.\\n'\n",
    )
    .await?;
    write_relative(
        state,
        GREENFIELD_USAGE_FILE,
        &format!(
            "# Docs-First Usage Examples\n\n\
             Before claiming product readiness, document the primary usage path for:\n\n\
             - first run\n\
             - expected success output\n\
             - common failure or recovery path\n\n\
             Goal: {}\n",
            state.normalized_goal
        ),
    )
    .await?;

    Ok(vec![
        ("greenfield_acceptance", GREENFIELD_ACCEPTANCE_FILE),
        ("greenfield_demo", GREENFIELD_DEMO_FILE),
        ("greenfield_usage_examples", GREENFIELD_USAGE_FILE),
    ])
}

async fn compatibility_plan(
    oracle: &super::super::oracle::GoalOracleEvidence,
    project_dir: &Path,
) -> Result<String> {
    if !matches!(
        oracle.kind,
        super::super::oracle::GoalKind::Rewrite
            | super::super::oracle::GoalKind::Migration
            | super::super::oracle::GoalKind::Refactor
    ) {
        return Ok(String::new());
    }

    let surfaces =
        super::super::oracle::surface::detect_source_project_surfaces(project_dir).await?;
    let command_lines = if surfaces.commands.is_empty() {
        "- No source-project commands detected.\n".to_string()
    } else {
        surfaces
            .commands
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect()
    };
    let api_lines = if surfaces.api_files.is_empty() {
        "- No source API files detected.\n".to_string()
    } else {
        surfaces
            .api_files
            .iter()
            .map(|path| format!("- `{}`\n", path.display()))
            .collect()
    };

    Ok(format!(
        "## Compatibility Test Plan\n\n\
         Source commands to preserve:\n\n\
         {command_lines}\n\
         Source API/file surfaces to compare:\n\n\
         {api_lines}\n"
    ))
}

async fn write_relative(state: &GoalState, relative: &str, contents: &str) -> Result<()> {
    crate::runtime::atomic::atomic_write(&state.state_dir.join(relative), contents.as_bytes()).await
}
