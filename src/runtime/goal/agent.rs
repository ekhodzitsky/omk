use super::state::{
    goal_agent_task_budget_secs, GoalState, GOAL_AGENT_EXECUTE_TASK_ID,
    GOAL_AGENT_FOLLOWUPS_RUN_ID, GOAL_AGENT_IMPLEMENT_TASK_ID, GOAL_AGENT_PUBLISH_TASK_ID,
    GOAL_AGENT_VERIFY_TASK_ID, GOAL_ARTIFACTS_DIR, GOAL_PRD_FILE, GOAL_PROOF_FILE,
    GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
use super::task_graph::{
    goal_task_done, pending_goal_agent_followup_proposals, GoalTaskGraph, GoalTaskStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GoalAgentTaskProposal {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) dependencies: Vec<String>,
    #[serde(default)]
    pub(crate) read_set: Vec<String>,
    #[serde(default)]
    pub(crate) write_set: Vec<String>,
    #[serde(default = "default_goal_agent_task_risk")]
    pub(crate) risk: String,
    #[serde(default)]
    pub(crate) acceptance: Vec<String>,
    #[serde(default = "default_goal_agent_task_budget_secs")]
    pub(crate) budget_secs: u64,
    #[serde(default)]
    pub(crate) priority: i32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalAgentRejectedTask {
    pub(crate) task: GoalAgentTaskProposal,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalAgentTaskPolicy {
    pub(crate) goal_id: String,
    pub(crate) run_id: String,
    pub(crate) max_agents: usize,
    pub(crate) proposed_tasks: Vec<GoalAgentTaskProposal>,
    pub(crate) accepted_tasks: Vec<GoalAgentTaskProposal>,
    pub(crate) rejected_tasks: Vec<GoalAgentRejectedTask>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GoalAgentWaveKind {
    Initial,
    FollowUp,
}

#[derive(Debug, Clone)]
pub(crate) struct GoalAgentDispatchPlan {
    pub(crate) run_key: String,
    pub(crate) kind: GoalAgentWaveKind,
    pub(crate) proposals: Vec<GoalAgentTaskProposal>,
    pub(crate) allow_existing_task_ids: bool,
}

fn default_goal_agent_task_risk() -> String {
    "moderate".to_string()
}

fn default_goal_agent_task_budget_secs() -> u64 {
    300
}

pub(crate) fn propose_goal_agent_tasks(state: &GoalState) -> Vec<GoalAgentTaskProposal> {
    vec![
        GoalAgentTaskProposal {
            id: GOAL_AGENT_IMPLEMENT_TASK_ID.to_string(),
            title: "Implement bounded goal slice".to_string(),
            description: "Make one bounded in-repository project change that moves the goal forward, then summarize changed files and remaining verification needs.".to_string(),
            dependencies: Vec::new(),
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
            ],
            write_set: vec![
                "project files".to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                GOAL_PROOF_FILE.to_string(),
            ],
            risk: "moderate".to_string(),
            acceptance: vec![
                "Make only a bounded project change needed to move the goal forward.".to_string(),
                "Do not commit, publish, or touch secrets.".to_string(),
                "Summarize changed files and verification still needed for production readiness.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 900),
            priority: 20,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_VERIFY_TASK_ID.to_string(),
            title: "Verify agent wave follow-up".to_string(),
            description: "Inspect the bounded implementation result and summarize verification, review, or hardening follow-up that still blocks readiness.".to_string(),
            dependencies: vec![GOAL_AGENT_IMPLEMENT_TASK_ID.to_string()],
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                "project files".to_string(),
            ],
            write_set: vec![GOAL_ARTIFACTS_DIR.to_string()],
            risk: "low".to_string(),
            acceptance: vec![
                "Review the bounded project change and call out remaining verification gaps.".to_string(),
                "Do not make broad follow-up mutations without a new controller-approved task.".to_string(),
                "Keep the goal proof honest when production readiness is still blocked.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 10,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_PUBLISH_TASK_ID.to_string(),
            title: "Publish crate release".to_string(),
            description: "Publish this crate to crates.io after the agent wave succeeds.".to_string(),
            dependencies: vec![GOAL_AGENT_VERIFY_TASK_ID.to_string()],
            read_set: vec![GOAL_PROOF_FILE.to_string()],
            write_set: vec!["crates.io".to_string()],
            risk: "external-side-effect".to_string(),
            acceptance: vec!["Publish the package to crates.io.".to_string()],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 0,
        },
    ]
}

pub(crate) fn goal_agent_dispatch_plan(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
) -> Option<GoalAgentDispatchPlan> {
    if !goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID) {
        return Some(GoalAgentDispatchPlan {
            run_key: GOAL_AGENT_EXECUTE_TASK_ID.to_string(),
            kind: GoalAgentWaveKind::Initial,
            proposals: propose_goal_agent_tasks(state),
            allow_existing_task_ids: false,
        });
    }

    let proposals = pending_goal_agent_followup_proposals(task_graph);
    (!proposals.is_empty()).then(|| GoalAgentDispatchPlan {
        run_key: GOAL_AGENT_FOLLOWUPS_RUN_ID.to_string(),
        kind: GoalAgentWaveKind::FollowUp,
        proposals,
        allow_existing_task_ids: true,
    })
}

pub(crate) fn validate_goal_agent_task_proposals(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    run_id: &str,
    proposals: Vec<GoalAgentTaskProposal>,
    allow_existing_task_ids: bool,
) -> GoalAgentTaskPolicy {
    let existing_task_ids: HashSet<String> = task_graph
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect();
    let mut known_dependencies: HashSet<String> = task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Done)
        .map(|task| task.id.clone())
        .collect();
    let mut seen_ids = HashSet::new();
    let mut accepted_tasks = Vec::new();
    let mut rejected_tasks = Vec::new();

    for proposal in proposals.iter().cloned() {
        if let Some(reason) = reject_goal_agent_task_proposal(
            &proposal,
            &mut seen_ids,
            &known_dependencies,
            &existing_task_ids,
            allow_existing_task_ids,
        ) {
            rejected_tasks.push(GoalAgentRejectedTask {
                task: proposal,
                reason,
            });
            continue;
        }

        known_dependencies.insert(proposal.id.clone());
        accepted_tasks.push(proposal);
    }

    GoalAgentTaskPolicy {
        goal_id: state.goal_id.clone(),
        run_id: run_id.to_string(),
        max_agents: state.max_agents.unwrap_or(1).max(1),
        proposed_tasks: proposals,
        accepted_tasks,
        rejected_tasks,
    }
}

fn reject_goal_agent_task_proposal(
    proposal: &GoalAgentTaskProposal,
    seen_ids: &mut HashSet<String>,
    known_dependencies: &HashSet<String>,
    existing_task_ids: &HashSet<String>,
    allow_existing_task_ids: bool,
) -> Option<String> {
    if proposal.id.trim().is_empty() {
        return Some("task id must not be empty".to_string());
    }
    if proposal.title.trim().is_empty() {
        return Some("task title must not be empty".to_string());
    }
    if proposal.description.trim().is_empty() {
        return Some("task description must not be empty".to_string());
    }
    if !seen_ids.insert(proposal.id.clone()) {
        return Some("duplicate task id rejected by goal policy".to_string());
    }
    if !allow_existing_task_ids && existing_task_ids.contains(&proposal.id) {
        return Some("task id already exists in the goal graph".to_string());
    }
    if proposal.budget_secs == 0 {
        return Some("task budget must be greater than zero".to_string());
    }

    let policy_text = format!(
        "{} {} {}",
        proposal.id,
        proposal.description,
        proposal.write_set.join(" ")
    )
    .to_ascii_lowercase();
    if policy_text.contains("crates.io") || policy_text.contains("publish") {
        return Some("publishing is disabled for GitHub-only goal execution".to_string());
    }

    if let Some(path) = proposal
        .read_set
        .iter()
        .chain(proposal.write_set.iter())
        .find(|path| !super::state::is_safe_goal_agent_path(path))
    {
        return Some(format!(
            "path is outside the allowed goal policy roots: {path}"
        ));
    }

    if let Some(dep) = proposal
        .dependencies
        .iter()
        .find(|dep| !known_dependencies.contains(*dep))
    {
        return Some(format!("dependency is not accepted or completed: {dep}"));
    }

    None
}

pub(crate) async fn append_goal_agent_task_policy_events(
    writer: &crate::runtime::events::EventWriter,
    run_id: &str,
    policy: &GoalAgentTaskPolicy,
) -> anyhow::Result<()> {
    for proposal in &policy.proposed_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskProposed,
        )
        .with_actor(super::state::GOAL_CONTROLLER_ACTOR)
        .with_payload(goal_agent_task_policy_payload(proposal, None))?;
        writer.append(&event).await?;
    }

    for proposal in &policy.accepted_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskAccepted,
        )
        .with_actor(super::state::GOAL_CONTROLLER_ACTOR)
        .with_payload(goal_agent_task_policy_payload(
            proposal,
            Some("accepted by goal policy"),
        ))?;
        writer.append(&event).await?;
    }

    for decision in &policy.rejected_tasks {
        let event = crate::runtime::events::Event::new(
            crate::runtime::events::RunId(run_id.to_string()),
            crate::runtime::events::EventKind::TaskRejected,
        )
        .with_actor(super::state::GOAL_CONTROLLER_ACTOR)
        .with_payload(goal_agent_task_policy_payload(
            &decision.task,
            Some(&decision.reason),
        ))?;
        writer.append(&event).await?;
    }

    Ok(())
}

pub(crate) fn goal_agent_task_policy_payload(
    proposal: &GoalAgentTaskProposal,
    reason: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "task_id": proposal.id,
        "title": proposal.title,
        "risk": proposal.risk,
        "budget_secs": proposal.budget_secs,
        "dependencies": proposal.dependencies,
        "read_set": proposal.read_set,
        "write_set": proposal.write_set,
        "reason": reason,
    })
}
