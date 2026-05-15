use tempfile::TempDir;

use crate::runtime::scheduler::task::{Task, TaskState};

use super::*;

#[tokio::test]
async fn test_seed_task_and_dispatch_writes_inbox() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Implement feature X");

    let spec = make_spec(&tmp, "worker-0").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let inbox = read_inbox(&spec).await;
    assert!(inbox.contains("task-1"));
    assert!(inbox.contains("Implement feature X"));

    assert_task_state(&runner, "task-1", TaskState::Running, Some("worker-0"));
}

#[tokio::test]
async fn test_dispatch_includes_structured_task_budget_in_inbox() {
    let tmp = TempDir::new().unwrap();
    let mut task =
        Task::new("task-budgeted", "budgeted task").with_description("honor this task budget");
    task.extra
        .insert("budget_secs".to_string(), serde_json::json!(7));

    let mut runner = make_runner_with_tasks(&tmp, vec![task]).await;

    let spec = make_spec(&tmp, "worker-budget").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let inbox = read_inbox(&spec).await;
    let task_json: serde_json::Value = serde_json::from_str(inbox.lines().next().unwrap()).unwrap();
    assert_eq!(task_json["id"], "task-budgeted");
    assert_eq!(task_json["budget_secs"], 7);
}

#[tokio::test]
async fn test_dispatch_blocks_conflicting_write_sets() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner_with_tasks(
        &tmp,
        vec![
            Task::new("task-1", "first writer")
                .with_description("write shared file first")
                .with_write_set(vec!["src/shared.rs".to_string()]),
            Task::new("task-2", "second writer")
                .with_description("write shared file second")
                .with_write_set(vec!["src/shared.rs".to_string()]),
        ],
    )
    .await;

    let worker_a = make_spec(&tmp, "worker-a").await;
    let worker_b = make_spec(&tmp, "worker-b").await;
    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    assert_task_state(&runner, "task-1", TaskState::Running, Some("worker-a"));
    assert_task_state(&runner, "task-2", TaskState::Pending, None);

    let inbox_a = read_inbox(&worker_a).await;
    assert!(inbox_a.contains("task-1"));
    assert!(
        !worker_b.inbox.exists(),
        "conflicting task must not be dispatched to the second worker"
    );
}
