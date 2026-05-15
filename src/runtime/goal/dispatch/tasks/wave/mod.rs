use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::runtime::config::{ensure_private_dir, EVENTS_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::events::{EventBuilder, EventWriter, RunId};
use crate::runtime::goal::agent::GoalAgentDispatchPlan;
use crate::runtime::goal::dispatch::runtime::{
    goal_agent_wire_runtime_available, goal_agent_worker_name,
};
use crate::runtime::goal::dispatch::tasks::scheduler::goal_agent_scheduler_tasks;
use crate::runtime::goal::evidence::{write_goal_agent_mutation_snapshot, GoalAgentRunEvidence};
use crate::runtime::goal::state::{
    GoalState, GOAL_AGENT_RUNS_DIR, GOAL_AGENT_TASK_POLICY_FILE,
    GOAL_AGENT_TASK_PROPOSALS_FILE, GOAL_ARTIFACTS_DIR,
};
use crate::runtime::goal::task_graph::GoalTaskGraph;
use crate::runtime::scheduler::runner::RunSummary;

mod policy;
mod results;
mod runner;

pub(crate) async fn run_goal_agent_task_wave(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    project_dir: &Path,
    started_at: DateTime<Utc>,
    dispatch: &GoalAgentDispatchPlan,
) -> Result<GoalAgentRunEvidence> {
    let run_id = format!("{}-{}", state.goal_id, dispatch.run_key);
    let run_path = PathBuf::from(GOAL_ARTIFACTS_DIR).join(GOAL_AGENT_RUNS_DIR).join(&dispatch.run_key);
    let run_dir = state.state_dir.join(&run_path);
    ensure_private_dir(&run_dir).await?;

    let primary = goal_agent_worker_name(0);
    let worker_outbox_path = run_path.join(WORKERS_DIR).join(&primary).join(OUTBOX_FILE);
    let wire_events_path = run_path.join(WORKERS_DIR).join(&primary).join("wire-events.jsonl");
    let task_policy_path = run_path.join(GOAL_AGENT_TASK_POLICY_FILE);
    let agent_task_proposals_path = run_path.join(GOAL_AGENT_TASK_PROPOSALS_FILE);
    let mutation_diff_path = run_path.join("mutation.diff");
    let changed_files_path = run_path.join("changed-files.json");

    let event_writer = EventWriter::new(state.state_dir.join(EVENTS_FILE));
    let builder = EventBuilder::new(RunId(run_id.clone()));
    let (dispatch_accepted, dispatch_rejected_count, policy) = policy::validate_and_classify_tasks(
        state, task_graph, &run_id, dispatch, &event_writer, &task_policy_path,
    )
    .await?;

    let accepted_task_ids: Vec<String> = dispatch_accepted.iter().map(|t| t.id.clone()).collect();
    let accepted_task_count = dispatch_accepted.len();
    let rejected_task_count = dispatch_rejected_count;
    let scheduler_tasks = goal_agent_scheduler_tasks(
        state, task_graph, started_at, &dispatch.run_key, &dispatch_accepted,
    );
    let run_description = format!(
        "goal controller agent wave {}: accepted={}, rejected={}, max_agents={}",
        dispatch.run_key, accepted_task_count, rejected_task_count, policy.max_agents
    );

    event_writer.append(&builder.run_started("goal-agent", project_dir, &run_description)?).await?;

    if scheduler_tasks.is_empty() {
        let s = "Goal controller rejected all proposed agent tasks; no safe work is dispatchable";
        event_writer.append(&builder.run_failed(s)?).await?;
        let changed_files = write_goal_agent_mutation_snapshot(state, project_dir, &mutation_diff_path, &changed_files_path).await?;
        return Ok(GoalAgentRunEvidence {
            summary: RunSummary { run_id, completed: 0, failed: 1, cancelled: 0, total: 1 },
            run_path, task_policy_path, agent_task_proposals_path, worker_outbox_path,
            wire_events_path, mutation_diff_path, changed_files_path, changed_files,
            accepted_task_count, rejected_task_count, accepted_task_ids,
            agent_proposed_tasks: Vec::new(), worker_results: Vec::new(),
            worker_summary: Some(s.to_string()),
        });
    }

    if !goal_agent_wire_runtime_available() {
        let s = "Kimi CLI not found; install/authenticate kimi or set MOCK_KIMI to a mock binary before running goal agent execution";
        event_writer.append(&builder.run_failed(s)?).await?;
        let changed_files = write_goal_agent_mutation_snapshot(state, project_dir, &mutation_diff_path, &changed_files_path).await?;
        return Ok(GoalAgentRunEvidence {
            summary: RunSummary { run_id, completed: 0, failed: accepted_task_count, cancelled: 0, total: accepted_task_count },
            run_path, task_policy_path, agent_task_proposals_path, worker_outbox_path,
            wire_events_path, mutation_diff_path, changed_files_path, changed_files,
            accepted_task_count, rejected_task_count, accepted_task_ids,
            agent_proposed_tasks: Vec::new(), worker_results: Vec::new(),
            worker_summary: Some(s.to_string()),
        });
    }

    let (summary, worker_specs) = runner::execute_wave_run(
        &run_id, project_dir, &run_dir, &state.state_dir, event_writer, &builder,
        scheduler_tasks, policy.max_agents,
    )
    .await?;

    let (worker_results, worker_summary, agent_proposed_tasks, changed_files) =
        results::gather_wave_results(
            &worker_specs, &accepted_task_ids, state, project_dir,
            &mutation_diff_path, &changed_files_path, &summary,
        )
        .await?;

    Ok(GoalAgentRunEvidence {
        summary, run_path, task_policy_path, agent_task_proposals_path, worker_outbox_path,
        wire_events_path, mutation_diff_path, changed_files_path, changed_files,
        accepted_task_count, rejected_task_count, accepted_task_ids,
        agent_proposed_tasks, worker_results, worker_summary,
    })
}
