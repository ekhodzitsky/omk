mod apply;
mod rollback;
mod types;

pub use types::GoalTaskGraphSummary;

pub(crate) use apply::{
    apply_agent_execution_task_result, apply_agent_followup_task_results,
    apply_agent_proposed_task_mutations, apply_agent_task_result_by_id, goal_agent_execution_done,
    goal_task_done, pending_goal_agent_followup_proposals, summarize_task_graph,
};
pub(crate) use rollback::{
    merge_concurrent_slice_task_graphs, spawn_cleanup_task, spawn_refactor_task_from_slop_findings,
};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;

    use super::*;
    use crate::runtime::goal::evidence::GoalAgentRunEvidence;
    use crate::runtime::goal::review::slop::{SlopFinding, SlopKind};
    use crate::runtime::goal::task_graph::model::{GoalTask, GoalTaskGraph, GoalTaskStatus};
    use crate::runtime::scheduler::runner::RunSummary;

    fn task(id: &str, status: GoalTaskStatus) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status,
            owner_role: None,
            completed_at: None,
            evidence: vec![],
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: vec![],
            read_set: vec![],
            write_set: vec!["src".to_string()],
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
        GoalTaskGraph {
            version: 1,
            goal_id: "goal-test".to_string(),
            generated_at: Utc::now(),
            tasks,
        }
    }

    fn evidence(task_id: &str, completed: usize, failed: usize) -> GoalAgentRunEvidence {
        GoalAgentRunEvidence {
            summary: RunSummary {
                run_id: format!("run-{task_id}"),
                completed,
                failed,
                cancelled: 0,
                total: completed + failed,
            },
            run_path: PathBuf::new(),
            task_policy_path: PathBuf::new(),
            agent_task_proposals_path: PathBuf::new(),
            worker_outbox_path: PathBuf::new(),
            wire_events_path: PathBuf::new(),
            mutation_diff_path: PathBuf::new(),
            changed_files_path: PathBuf::new(),
            changed_files: vec![],
            accepted_task_count: 0,
            rejected_task_count: 0,
            accepted_task_ids: vec![task_id.to_string()],
            agent_proposed_tasks: vec![],
            worker_results: vec![],
            worker_summary: None,
        }
    }

    #[test]
    fn merge_updates_status_to_done() {
        let mut main = graph(vec![
            task("t1", GoalTaskStatus::Pending),
            task("t2", GoalTaskStatus::Pending),
        ]);
        let mut delta1 = graph(vec![task("t1", GoalTaskStatus::Done)]);
        delta1.tasks[0].completed_at = Some(Utc::now());
        let mut delta2 = graph(vec![task("t2", GoalTaskStatus::Done)]);
        delta2.tasks[0].completed_at = Some(Utc::now());

        merge_concurrent_slice_task_graphs(&mut main, &[delta1, delta2]);

        assert_eq!(main.tasks[0].status, GoalTaskStatus::Done);
        assert_eq!(main.tasks[1].status, GoalTaskStatus::Done);
    }

    #[test]
    fn merge_prefers_blocked_over_pending() {
        let mut main = graph(vec![task("t1", GoalTaskStatus::Pending)]);
        let delta = graph(vec![task("t1", GoalTaskStatus::Blocked)]);

        merge_concurrent_slice_task_graphs(&mut main, &[delta]);

        assert_eq!(main.tasks[0].status, GoalTaskStatus::Blocked);
    }

    #[test]
    fn apply_agent_task_result_by_id_sets_done() {
        let mut tg = graph(vec![task("t1", GoalTaskStatus::Pending)]);
        let ev = evidence("t1", 1, 0);
        let result = apply_agent_task_result_by_id(&mut tg, "t1", &ev, Utc::now());
        assert!(result.is_some());
        assert_eq!(tg.tasks[0].status, GoalTaskStatus::Done);
    }

    #[test]
    fn spawn_refactor_task_from_empty_findings_returns_none() {
        let mut tg = graph(vec![task("slice-a", GoalTaskStatus::Done)]);
        let result = spawn_refactor_task_from_slop_findings(
            &mut tg,
            "slice-a",
            &[],
            &["src/main.rs".to_string()],
            Utc::now(),
        );
        assert!(result.is_none());
        assert_eq!(tg.tasks.len(), 1);
    }

    #[test]
    fn spawn_refactor_task_creates_task_and_wires_dependency() {
        let mut tg = graph(vec![task("slice-a", GoalTaskStatus::Done)]);
        let findings = vec![SlopFinding {
            kind: SlopKind::BannedPattern,
            file: PathBuf::from("src/main.rs"),
            line: Some(42),
            message: "unwrap() is banned".to_string(),
        }];
        let result = spawn_refactor_task_from_slop_findings(
            &mut tg,
            "slice-a",
            &findings,
            &["src/main.rs".to_string()],
            Utc::now(),
        );
        assert_eq!(result, Some("goal-agent-refactor-slice-a".to_string()));
        assert_eq!(tg.tasks.len(), 2);

        let refactor = tg
            .tasks
            .iter()
            .find(|t| t.id == "goal-agent-refactor-slice-a")
            .expect("refactor task exists");
        assert!(refactor.description.contains("unwrap() is banned"));
        assert!(refactor.description.contains("line 42"));
        assert_eq!(refactor.status, GoalTaskStatus::Pending);
        assert_eq!(refactor.read_set, vec!["src/main.rs"]);

        let slice = tg
            .tasks
            .iter()
            .find(|t| t.id == "slice-a")
            .expect("slice task exists");
        assert!(slice
            .dependencies
            .contains(&"goal-agent-refactor-slice-a".to_string()));
        assert_eq!(slice.status, GoalTaskStatus::Pending);
    }

    #[test]
    fn spawn_refactor_task_updates_existing_task_when_findings_change() {
        let mut tg = graph(vec![task("slice-a", GoalTaskStatus::Done)]);
        let first = vec![SlopFinding {
            kind: SlopKind::TodoFixmeHack,
            file: PathBuf::from("src/lib.rs"),
            line: Some(10),
            message: "TODO fix this".to_string(),
        }];
        let id1 = spawn_refactor_task_from_slop_findings(
            &mut tg,
            "slice-a",
            &first,
            &["src/lib.rs".to_string()],
            Utc::now(),
        );
        assert!(id1.is_some());

        let second = vec![SlopFinding {
            kind: SlopKind::BannedPattern,
            file: PathBuf::from("src/lib.rs"),
            line: Some(20),
            message: "expect() is banned".to_string(),
        }];
        let id2 = spawn_refactor_task_from_slop_findings(
            &mut tg,
            "slice-a",
            &second,
            &["src/lib.rs".to_string()],
            Utc::now(),
        );
        assert_eq!(id1, id2);
        assert_eq!(tg.tasks.len(), 2);

        let refactor = tg
            .tasks
            .iter()
            .find(|t| t.id == "goal-agent-refactor-slice-a")
            .expect("refactor task exists");
        assert!(refactor.description.contains("expect() is banned"));
        assert!(!refactor.description.contains("TODO fix this"));
        assert_eq!(refactor.evidence.len(), 2);
    }
}
