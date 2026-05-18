use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::runtime::events::{EventBuilder, EventWriter, RunId};
use crate::runtime::goal::evidence::{detect_git_evidence, record_artifact};
use crate::runtime::goal::proof::{build_scaffold_proof, write_json_artifact};
use crate::runtime::goal::state::{
    FileSystemGoalStateStore, GoalState, GoalStateStore, GoalFailure, GoalPhase, GoalStatus,
    GOAL_AGENT_EXECUTE_TASK_ID, GOAL_AGENT_VERIFY_TASK_ID, GOAL_ARTIFACTS_DIR,
    GOAL_DECISIONS_FILE, GOAL_FAILURE_FILE, GOAL_LOCAL_VERIFY_TASK_ID,
    GOAL_PRD_FILE, GOAL_PROOF_FILE, GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE,
    GOAL_TEST_SPEC_FILE,
};
use crate::runtime::goal::task_graph::{GoalTaskGraph, GoalTaskStatus};

pub(crate) async fn create_goal_with_scaffold(
    goal: &str,
    options: crate::runtime::goal::state::CreateGoalOptions,
) -> anyhow::Result<crate::runtime::goal::state::GoalState> {
    let id = crate::runtime::goal::types::GoalId::generate();
    let id_string = id.to_string();
    let until_ready = options.until_ready;
    let slice_execution = options.slice_execution;
    let delivery_policy = options.delivery_policy;
    let merge_policy = options.merge_policy;
    let budget = crate::runtime::goal::types::GoalBudget::from_options(options)?;
    let goal_dir = crate::runtime::goal::state::goals_dir().join(id.as_str());
    crate::runtime::config::ensure_private_dir(&goal_dir).await?;

    let now = chrono::Utc::now();
    let normalized_goal = crate::runtime::goal::state::normalize_goal(goal);
    let oracle = crate::runtime::goal::oracle::assess_goal_oracle(&normalized_goal);
    let failure = (!oracle.testable).then(|| GoalFailure {
        reason: oracle.human_decisions_required.join("; "),
        recorded_at: now,
    });
    let state = crate::runtime::goal::state::GoalState {
        version: 1,
        goal_id: id_string,
        original_goal: goal.to_string(),
        normalized_goal,
        status: if oracle.testable {
            crate::runtime::goal::state::GoalStatus::NotReady
        } else {
            crate::runtime::goal::state::GoalStatus::BlockedOnHuman
        },
        phase: crate::runtime::goal::state::GoalPhase::Intake,
        created_at: now,
        updated_at: now,
        completed_at: Some(now),
        until_ready,
        budget_time: budget.time,
        budget_tokens: budget.tokens,
        budget_usd: budget.usd,
        max_agents: budget.max_agents,
        terminal_criteria: crate::runtime::goal::state::GoalTerminalCriteria::default(),
        artifacts: Vec::new(),
        failure,
        state_dir: goal_dir.clone(),
        cost_tracker_path: Some(goal_dir.join("cost.jsonl")),
        delivery_policy,
        merge_policy,
        slice_execution,
    };
    FileSystemGoalStateStore::new().save(&state).await?;

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
    super::super::artifacts::write_goal_brief(&state, now).await?;
    record_artifact(&mut state, "prd", GOAL_PRD_FILE, now);

    state.phase = GoalPhase::Planning;
    let relevant_files = super::super::discover::discover_relevant_files(&state.normalized_goal, &cwd)?;
    super::super::artifacts::write_technical_plan(&state, &relevant_files, now).await?;
    record_artifact(&mut state, "technical_plan", GOAL_TECHNICAL_PLAN_FILE, now);

    state.phase = GoalPhase::Decomposition;
    let task_graph = write_task_graph(&state, now).await?;
    record_artifact(&mut state, "task_graph", GOAL_TASK_GRAPH_FILE, now);

    state.phase = GoalPhase::VerificationDesign;
    super::super::artifacts::write_test_spec(&state, &task_graph, &cwd, now).await?;
    record_artifact(&mut state, "test_spec", GOAL_TEST_SPEC_FILE, now);
    for (kind, path) in super::super::artifacts::write_greenfield_oracle_artifacts(&state).await? {
        record_artifact(&mut state, kind, path, now);
    }
    super::append_controller_task_events(&state, &task_graph).await?;
    if state.until_ready && state.status != GoalStatus::BlockedOnHuman {
        super::super::delivery::materialize_delivery_slices(&state, &task_graph, &cwd).await?;
    }

    state.phase = GoalPhase::Proof;
    let git = detect_git_evidence(&cwd).await;
    let proof = build_scaffold_proof(&state, &task_graph, git, now);
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;
    record_artifact(&mut state, "proof", GOAL_PROOF_FILE, now);
    crate::runtime::goal::decision::append_controller_scaffold_decisions(&state, &task_graph, now)
        .await?;
    record_artifact(&mut state, "decisions", GOAL_DECISIONS_FILE, now);

    if state.status != GoalStatus::BlockedOnHuman {
        state.status = GoalStatus::NotReady;
    }
    state.updated_at = now;
    state.completed_at = Some(now);
    FileSystemGoalStateStore::new().save(&state).await?;
    if state.status == GoalStatus::BlockedOnHuman {
        write_blocked_goal_failure_artifact(&state).await?;
    }
    crate::runtime::goal::budget::append_budget_checkpoint(&state, "goal_created").await?;

    let run_failed_reason = if state.status == GoalStatus::BlockedOnHuman {
        state
            .failure
            .as_ref()
            .map(|failure| failure.reason.as_str())
            .unwrap_or("blocked_on_human")
    } else {
        "goal controller scaffold created; run omk goal execute to launch the bounded agent wave"
    };
    writer
        .append(&builder.run_failed(run_failed_reason)?)
        .await?;

    Ok(state)
}

async fn write_blocked_goal_failure_artifact(state: &GoalState) -> Result<()> {
    let failure_json = serde_json::to_string_pretty(state)?;
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(GOAL_FAILURE_FILE),
        failure_json.as_bytes(),
    )
    .await
}

async fn write_task_graph(state: &GoalState, generated_at: DateTime<Utc>) -> Result<GoalTaskGraph> {
    let mut tasks = vec![
        super::scaffold_intake_task(generated_at),
        super::scaffold_plan_task(generated_at),
        super::scaffold_local_verify_task(),
    ];

    let slice_mode = state.slice_execution;
    let max_features = state.max_agents.unwrap_or(2).max(2);
    let features = if slice_mode {
        super::super::decompose_goal_for_slices(&state.normalized_goal, max_features)
    } else {
        Vec::new()
    };

    let agent_verify_dependency = if slice_mode && features.len() > 1 {
        let mut implement_task_ids = Vec::new();
        for (i, feature) in features.iter().enumerate() {
            let slug = super::super::sanitize_feature_slug(feature);
            let task_id = format!("goal-agent-implement-{i}");
            implement_task_ids.push(task_id.clone());
            tasks.push(crate::runtime::goal::task_graph::GoalTask {
                id: task_id,
                title: format!("Implement: {feature}"),
                description: format!(
                    "Implement {feature} as part of the goal. Write changes to src/{slug}/."
                ),
                status: GoalTaskStatus::Pending,
                owner_role: Some("executor".to_string()),
                completed_at: None,
                evidence: Vec::new(),
                retry_count: 0,
                max_retries: 0,
                lease_expires_at: None,
                dependencies: vec![GOAL_LOCAL_VERIFY_TASK_ID.to_string()],
                read_set: vec![
                    GOAL_PRD_FILE.to_string(),
                    GOAL_TECHNICAL_PLAN_FILE.to_string(),
                    GOAL_TEST_SPEC_FILE.to_string(),
                    GOAL_TASK_GRAPH_FILE.to_string(),
                ],
                write_set: vec![format!("src/{slug}/")],
                risk: "moderate".to_string(),
                acceptance: vec![
                    format!("Implementation for {feature} is complete in src/{slug}/."),
                    "Do not commit, publish, or touch secrets.".to_string(),
                    "Summarize changed files and verification still needed.".to_string(),
                ],
            });
        }

        let mut verify_read_set = vec![
            GOAL_PRD_FILE.to_string(),
            GOAL_TECHNICAL_PLAN_FILE.to_string(),
            GOAL_TEST_SPEC_FILE.to_string(),
            GOAL_TASK_GRAPH_FILE.to_string(),
        ];
        for slug in features.iter().map(|f| super::super::sanitize_feature_slug(f)) {
            verify_read_set.push(format!("src/{slug}/"));
        }

        tasks.push(crate::runtime::goal::task_graph::GoalTask {
            id: GOAL_AGENT_VERIFY_TASK_ID.to_string(),
            title: "Verify agent implementations".to_string(),
            description: "Inspect all implementation results and summarize verification, review, or hardening follow-up that still blocks readiness.".to_string(),
            status: GoalTaskStatus::Pending,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: implement_task_ids,
            read_set: verify_read_set,
            write_set: vec![GOAL_ARTIFACTS_DIR.to_string()],
            risk: "low".to_string(),
            acceptance: vec![
                "Review all bounded project changes and call out remaining verification gaps.".to_string(),
                "Do not make broad follow-up mutations without a new controller-approved task.".to_string(),
                "Keep the goal proof honest when production readiness is still blocked.".to_string(),
            ],
        });
        GOAL_AGENT_VERIFY_TASK_ID.to_string()
    } else {
        tasks.push(super::scaffold_agent_execute_task());
        GOAL_AGENT_EXECUTE_TASK_ID.to_string()
    };

    tasks.push(super::scaffold_review_task(&agent_verify_dependency));
    tasks.push(super::scaffold_security_review_task(&agent_verify_dependency));

    let graph = GoalTaskGraph {
        version: 1,
        goal_id: state.goal_id.clone(),
        generated_at,
        tasks,
    };
    write_json_artifact(&state.state_dir.join(GOAL_TASK_GRAPH_FILE), &graph).await?;
    Ok(graph)
}
