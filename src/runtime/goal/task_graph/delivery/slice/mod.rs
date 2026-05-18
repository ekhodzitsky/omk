mod builder;
mod types;
mod validation;

pub use builder::{plan_goal_delivery_slices, record_goal_delivery_slice_plan};
pub use types::{GoalDeliveryOverlapSerialization, GoalDeliverySlice, GoalDeliverySlicePlan};
pub use validation::{all_slices_done, ready_delivery_slices};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph, GoalTaskStatus};

    fn task(id: &str, status: GoalTaskStatus, dependencies: &[&str]) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: dependencies.iter().map(|d| d.to_string()).collect(),
            read_set: Vec::new(),
            write_set: vec!["project files".to_string()],
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
        GoalTaskGraph {
            version: 1,
            goal_id: "goal-test".to_string(),
            generated_at: chrono::Utc::now(),
            tasks,
        }
    }

    #[tokio::test]
    async fn all_slices_done_returns_false_when_no_records() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph = graph(vec![task("t1", GoalTaskStatus::Pending, &[])]);
        let task_graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": chrono::Utc::now(),
            "tasks": [
                {
                    "id": "t1",
                    "title": "Task t1",
                    "description": "desc",
                    "status": "pending",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"]
                }
            ]
        });
        tokio::fs::write(
            tmp.path()
                .join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&task_graph_json).expect("json"),
        )
        .await
        .expect("write");

        let done = all_slices_done(tmp.path(), &graph)
            .await
            .expect("all_slices_done");
        assert!(!done, "no delivery records means not done");
    }

    #[tokio::test]
    async fn all_slices_done_returns_true_when_all_tasks_done() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph = graph(vec![
            task("t1", GoalTaskStatus::Done, &[]),
            task("t2", GoalTaskStatus::Done, &["t1"]),
        ]);
        let task_graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": chrono::Utc::now(),
            "tasks": [
                {
                    "id": "t1",
                    "title": "Task t1",
                    "description": "desc",
                    "status": "done",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": {
                        "slice_id": "t1",
                        "worktree_path": "/tmp/wt1",
                        "status": "delivered"
                    }
                },
                {
                    "id": "t2",
                    "title": "Task t2",
                    "description": "desc",
                    "status": "done",
                    "dependencies": ["t1"],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": {
                        "slice_id": "t2",
                        "worktree_path": "/tmp/wt2",
                        "status": "delivered"
                    }
                }
            ]
        });
        tokio::fs::write(
            tmp.path()
                .join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&task_graph_json).expect("json"),
        )
        .await
        .expect("write");

        let done = all_slices_done(tmp.path(), &graph)
            .await
            .expect("all_slices_done");
        assert!(done, "all slice tasks are done");
    }

    #[tokio::test]
    async fn ready_delivery_slices_filters_done_and_blocked_dependencies() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let graph = graph(vec![
            task("t1", GoalTaskStatus::Done, &[]),
            task("t2", GoalTaskStatus::Pending, &["t1"]),
            task("t3", GoalTaskStatus::Pending, &["t1"]),
        ]);
        let task_graph_json = serde_json::json!({
            "version": 1,
            "goal_id": "goal-test",
            "generated_at": chrono::Utc::now(),
            "tasks": [
                {
                    "id": "t1",
                    "title": "Task t1",
                    "description": "desc",
                    "status": "done",
                    "dependencies": [],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": { "slice_id": "t1", "worktree_path": "/tmp/wt1", "status": "delivered" }
                },
                {
                    "id": "t2",
                    "title": "Task t2",
                    "description": "desc",
                    "status": "pending",
                    "dependencies": ["t1"],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": { "slice_id": "t2", "worktree_path": "/tmp/wt2", "status": "planned", "dependencies": ["t1"] }
                },
                {
                    "id": "t3",
                    "title": "Task t3",
                    "description": "desc",
                    "status": "pending",
                    "dependencies": ["t1"],
                    "read_set": [],
                    "write_set": ["project files"],
                    "risk": "low",
                    "acceptance": ["a"],
                    "delivery": { "slice_id": "t3", "worktree_path": "/tmp/wt3", "status": "planned", "dependencies": ["t1", "t2"] }
                }
            ]
        });
        tokio::fs::write(
            tmp.path()
                .join(crate::runtime::goal::state::GOAL_TASK_GRAPH_FILE),
            serde_json::to_vec_pretty(&task_graph_json).expect("json"),
        )
        .await
        .expect("write");

        let ready = ready_delivery_slices(tmp.path(), &graph)
            .await
            .expect("ready");
        assert_eq!(ready.len(), 1, "only t2 is ready (t3 blocked on t2)");
        assert_eq!(ready[0].task_id, "t2");
        assert_eq!(ready[0].worktree_path, PathBuf::from("/tmp/wt2"));
    }
}
