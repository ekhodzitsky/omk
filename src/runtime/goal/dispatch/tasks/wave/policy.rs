use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::runtime::events::{Event, EventKind, EventWriter, RunId};
use crate::runtime::goal::agent::{
    check_task_path_policy, goal_agent_task_policy_payload, validate_goal_agent_task_proposals,
    GoalAgentDispatchPlan, GoalAgentTaskPolicy, GoalAgentTaskProposal,
};
use crate::runtime::goal::budget::evaluate_task_budget;
use crate::runtime::goal::dispatch::tasks::payload::{
    task_dispatch_accepted_payload, task_dispatch_rejected_payload,
};
use crate::runtime::goal::proof::write_json_artifact;
use crate::runtime::goal::state::{GoalState, GOAL_CONTROLLER_ACTOR};
use crate::runtime::goal::task_graph::GoalTaskGraph;

pub(crate) async fn validate_and_classify_tasks(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    run_id: &str,
    dispatch: &GoalAgentDispatchPlan,
    event_writer: &EventWriter,
    task_policy_path: &Path,
) -> Result<(Vec<GoalAgentTaskProposal>, usize, GoalAgentTaskPolicy)> {
    let policy = validate_goal_agent_task_proposals(
        state,
        task_graph,
        run_id,
        dispatch.proposals.clone(),
        dispatch.allow_existing_task_ids,
    );
    write_json_artifact(&state.state_dir.join(task_policy_path), &policy).await?;

    let wave_rejected: HashMap<String, String> = policy
        .rejected_tasks
        .iter()
        .map(|r| (r.task.id.clone(), r.reason.clone()))
        .collect();

    let mut dispatch_accepted = Vec::new();
    let mut dispatch_rejected_count = 0;

    for proposal in &policy.proposed_tasks {
        let proposed_event = Event::new(RunId(run_id.to_string()), EventKind::TaskProposed)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(goal_agent_task_policy_payload(proposal, None))?;
        event_writer.append(&proposed_event).await?;

        if let Some(reason) = wave_rejected.get(&proposal.id) {
            let rejected_event = Event::new(RunId(run_id.to_string()), EventKind::TaskRejected)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(task_dispatch_rejected_payload(proposal, reason, None)?)?;
            event_writer.append(&rejected_event).await?;
            dispatch_rejected_count += 1;
            continue;
        }

        match evaluate_task_budget(state, proposal).await {
            Ok(snapshot) => {
                if let Some(reason) = check_task_path_policy(proposal) {
                    let rejected_event =
                        Event::new(RunId(run_id.to_string()), EventKind::TaskRejected)
                            .with_actor(GOAL_CONTROLLER_ACTOR)
                            .with_payload(task_dispatch_rejected_payload(
                                proposal,
                                &reason,
                                Some(&snapshot),
                            )?)?;
                    event_writer.append(&rejected_event).await?;
                    dispatch_rejected_count += 1;
                } else {
                    let accepted_event =
                        Event::new(RunId(run_id.to_string()), EventKind::TaskAccepted)
                            .with_actor(GOAL_CONTROLLER_ACTOR)
                            .with_payload(task_dispatch_accepted_payload(proposal, &snapshot)?)?;
                    event_writer.append(&accepted_event).await?;
                    dispatch_accepted.push(proposal.clone());
                }
            }
            Err(reason) => {
                let rejected_event =
                    Event::new(RunId(run_id.to_string()), EventKind::TaskRejected)
                        .with_actor(GOAL_CONTROLLER_ACTOR)
                        .with_payload(task_dispatch_rejected_payload(proposal, &reason, None)?)?;
                event_writer.append(&rejected_event).await?;
                dispatch_rejected_count += 1;
            }
        }
    }

    Ok((dispatch_accepted, dispatch_rejected_count, policy))
}
