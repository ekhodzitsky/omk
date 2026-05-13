use super::evidence::{detect_git_evidence, record_artifact};
use super::proof::{build_scaffold_proof, write_json_artifact};
use super::state::{
    GoalPhase, GoalState, GoalStatus, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_AGENT_RUNS_DIR,
    GOAL_ARTIFACTS_DIR, GOAL_CONTROLLER_ACTOR, GOAL_GATE_ARTIFACTS_DIR, GOAL_LOCAL_VERIFY_TASK_ID,
    GOAL_PRD_FILE, GOAL_PROOF_FILE, GOAL_REVIEW_ARTIFACTS_DIR, GOAL_REVIEW_FILE,
    GOAL_REVIEW_TASK_ID, GOAL_SECURITY_REVIEW_FILE, GOAL_SECURITY_REVIEW_TASK_ID,
    GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
};
use super::task_graph::{GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

pub(crate) async fn create_goal_with_scaffold(
    goal: &str,
    options: super::state::CreateGoalOptions,
) -> anyhow::Result<super::state::GoalState> {
    let id = super::state::generate_goal_id();
    let goal_dir = super::state::goals_dir().join(&id);
    crate::runtime::config::ensure_private_dir(&goal_dir).await?;

    let now = chrono::Utc::now();
    let state = super::state::GoalState {
        version: 1,
        goal_id: id.clone(),
        original_goal: goal.to_string(),
        normalized_goal: super::state::normalize_goal(goal),
        status: super::state::GoalStatus::NotReady,
        phase: super::state::GoalPhase::Intake,
        created_at: now,
        updated_at: now,
        completed_at: Some(now),
        until_ready: options.until_ready,
        budget_time: options.budget_time,
        max_agents: options.max_agents,
        terminal_criteria: super::state::GoalTerminalCriteria::default(),
        artifacts: Vec::new(),
        failure: None,
        state_dir: goal_dir.clone(),
    };
    state.save().await?;

    run_controller_scaffold(state).await
}

pub(crate) async fn run_controller_scaffold(mut state: GoalState) -> Result<GoalState> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    writer
        .append(&builder.run_started("goal", &cwd, &state.original_goal)?)
        .await?;

    let now = Utc::now();
    state.phase = GoalPhase::Planning;
    write_goal_brief(&state, now).await?;
    record_artifact(&mut state, "prd", GOAL_PRD_FILE, now);

    state.phase = GoalPhase::Planning;
    write_technical_plan(&state, now).await?;
    record_artifact(&mut state, "technical_plan", GOAL_TECHNICAL_PLAN_FILE, now);

    state.phase = GoalPhase::Decomposition;
    let task_graph = write_task_graph(&state, now).await?;
    record_artifact(&mut state, "task_graph", GOAL_TASK_GRAPH_FILE, now);

    state.phase = GoalPhase::VerificationDesign;
    write_test_spec(&state, &task_graph, now).await?;
    record_artifact(&mut state, "test_spec", GOAL_TEST_SPEC_FILE, now);
    append_controller_task_events(&state, &task_graph).await?;

    state.phase = GoalPhase::Proof;
    let git = detect_git_evidence(&cwd).await;
    let proof = build_scaffold_proof(&state, &task_graph, git, now);
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;
    record_artifact(&mut state, "proof", GOAL_PROOF_FILE, now);

    state.status = GoalStatus::NotReady;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;
    super::budget::append_budget_checkpoint(&state, "goal_created").await?;

    writer
        .append(
            &builder.run_failed(
                "goal controller scaffold created; run omk goal execute to launch the bounded agent wave",
            )?,
        )
        .await?;

    Ok(state)
}

async fn write_goal_brief(state: &GoalState, generated_at: DateTime<Utc>) -> Result<()> {
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

async fn write_technical_plan(state: &GoalState, generated_at: DateTime<Utc>) -> Result<()> {
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

async fn write_test_spec(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    generated_at: DateTime<Utc>,
) -> Result<()> {
    let task_lines = task_graph
        .tasks
        .iter()
        .map(|task| format!("- `{}`: {}", task.id, task.acceptance.join("; ")))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "# Test Spec\n\n\
         Generated: {generated_at}\n\n\
         ## Required Proof Before Ready\n\n\
         - Required gates must pass.\n\
         - A proof artifact must cite gate evidence.\n\
         - Known gaps must be empty or explicitly accepted by a human.\n\n\
         ## Scaffold Task Acceptance\n\n\
         {task_lines}\n\n\
         ## Current Status\n\n\
         `omk goal` remains `not_ready` because readiness still requires integration acceptance and specialist review loops beyond controller-owned planning, local verification, bounded agent execution, mutation evidence, post-mutation gate reruns, and controller review/security evidence.\n\n\
         Goal ID: `{}`\n",
        state.goal_id
    );
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(GOAL_TEST_SPEC_FILE),
        body.as_bytes(),
    )
    .await
}

async fn write_task_graph(state: &GoalState, generated_at: DateTime<Utc>) -> Result<GoalTaskGraph> {
    let graph = GoalTaskGraph {
        version: 1,
        goal_id: state.goal_id.clone(),
        generated_at,
        tasks: vec![
            GoalTask {
                id: "goal-intake".to_string(),
                title: "Clarify durable goal intent".to_string(),
                description: "Preserve the original request, normalized goal, assumptions, and current execution boundary.".to_string(),
                status: GoalTaskStatus::Done,
                owner_role: Some(GOAL_CONTROLLER_ACTOR.to_string()),
                completed_at: Some(generated_at),
                evidence: vec![GoalTaskEvidence {
                    kind: "artifact".to_string(),
                    path: PathBuf::from(GOAL_PRD_FILE),
                    summary: "Goal brief records the original and normalized goal.".to_string(),
                }],
                dependencies: Vec::new(),
                read_set: vec!["goal.json".to_string()],
                write_set: vec![GOAL_PRD_FILE.to_string()],
                risk: "low".to_string(),
                acceptance: vec![
                    "goal brief exists".to_string(),
                    "original and normalized goals are recorded".to_string(),
                ],
            },
            GoalTask {
                id: "goal-plan".to_string(),
                title: "Design controller work plan".to_string(),
                description: "Persist a technical plan and explicit verification expectations for the goal controller.".to_string(),
                status: GoalTaskStatus::Done,
                owner_role: Some(GOAL_CONTROLLER_ACTOR.to_string()),
                completed_at: Some(generated_at),
                evidence: vec![
                    GoalTaskEvidence {
                        kind: "artifact".to_string(),
                        path: PathBuf::from(GOAL_TECHNICAL_PLAN_FILE),
                        summary: "Technical plan records controller phases and execution boundary.".to_string(),
                    },
                    GoalTaskEvidence {
                        kind: "artifact".to_string(),
                        path: PathBuf::from(GOAL_TASK_GRAPH_FILE),
                        summary: "Task graph records the first controller-owned work graph.".to_string(),
                    },
                    GoalTaskEvidence {
                        kind: "artifact".to_string(),
                        path: PathBuf::from(GOAL_TEST_SPEC_FILE),
                        summary: "Test spec records readiness expectations.".to_string(),
                    },
                ],
                dependencies: vec!["goal-intake".to_string()],
                read_set: vec![GOAL_PRD_FILE.to_string()],
                write_set: vec![
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                ],
                risk: "medium".to_string(),
                acceptance: vec![
                    "technical plan exists".to_string(),
                    "test spec describes readiness gates".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_LOCAL_VERIFY_TASK_ID.to_string(),
                title: "Run local verification wall".to_string(),
                description: "Run configured local gates, capture gate evidence, and refresh the goal proof.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec!["goal-plan".to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                    ".omk/gates.toml".to_string(),
                ],
                write_set: vec![
                    format!("{GOAL_ARTIFACTS_DIR}/{GOAL_GATE_ARTIFACTS_DIR}"),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "medium".to_string(),
                acceptance: vec![
                    "all required gates pass".to_string(),
                    "proof cites gate evidence".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_AGENT_EXECUTE_TASK_ID.to_string(),
                title: "Execute agent-owned implementation tasks".to_string(),
                description: "Run bounded agent work through Wire, allow minimal project mutations, and capture mutation evidence before readiness can be claimed.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec![GOAL_LOCAL_VERIFY_TASK_ID.to_string()],
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
                risk: "high".to_string(),
                acceptance: vec![
                    "agent execution produces task and mutation evidence".to_string(),
                    "proof status becomes ready only with evidence".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_REVIEW_TASK_ID.to_string(),
                title: "Review goal execution evidence".to_string(),
                description: "Check that local verification and agent execution evidence are present before any readiness claim.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec![GOAL_AGENT_EXECUTE_TASK_ID.to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                    format!("{GOAL_ARTIFACTS_DIR}/{GOAL_AGENT_RUNS_DIR}"),
                ],
                write_set: vec![
                    format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_REVIEW_FILE}"),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "medium".to_string(),
                acceptance: vec![
                    "review artifact cites task and gate evidence".to_string(),
                    "review task remains blocked when execution evidence is missing".to_string(),
                ],
            },
            GoalTask {
                id: GOAL_SECURITY_REVIEW_TASK_ID.to_string(),
                title: "Run security evidence check".to_string(),
                description: "Run a bounded controller security review over goal evidence and changed files.".to_string(),
                status: GoalTaskStatus::Pending,
                owner_role: None,
                completed_at: None,
                evidence: Vec::new(),
                dependencies: vec![GOAL_AGENT_EXECUTE_TASK_ID.to_string()],
                read_set: vec![
                    GOAL_PROOF_FILE.to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    "changed files".to_string(),
                ],
                write_set: vec![
                    format!(
                        "{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_SECURITY_REVIEW_FILE}"
                    ),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                    GOAL_PROOF_FILE.to_string(),
                ],
                risk: "high".to_string(),
                acceptance: vec![
                    "security review artifact exists".to_string(),
                    "high-confidence secret findings block the task".to_string(),
                ],
            },
        ],
    };
    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &graph).await?;
    Ok(graph)
}

pub(crate) async fn append_controller_task_events(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let mut events = Vec::new();

    for task in task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Done)
    {
        let task_id = TaskId(task.id.clone());
        events.push(
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "title": task.title,
                }))?,
        );
        events.push(builder.task_completed(
            task_id,
            worker_id.clone(),
            Some(&controller_task_summary(task)),
        )?);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}

pub(crate) fn controller_task_summary(task: &GoalTask) -> String {
    let artifacts = task
        .evidence
        .iter()
        .map(|evidence| evidence.path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} completed with artifact evidence: {}",
        task.id, artifacts
    )
}
