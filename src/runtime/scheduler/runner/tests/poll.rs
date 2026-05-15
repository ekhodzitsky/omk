use tempfile::TempDir;

use crate::runtime::scheduler::task::TaskState;

use super::*;

#[tokio::test]
async fn test_poll_reads_worker_result_and_completes_task() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Implement feature X");

    let spec = make_spec(&tmp, "worker-0").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let result = success_result("task-1");
    write_outbox(
        &spec,
        format!("{}\n", serde_json::to_string(&result).unwrap()),
    )
    .await;

    runner.poll_workers().await.unwrap();

    assert_task_completed(&runner, "task-1");
}

#[tokio::test]
async fn test_poll_reads_simple_failed_result() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Implement feature X");

    let spec = make_spec(&tmp, "worker-0").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let result = simple_failed_json("task-1", "compilation error");
    write_outbox(&spec, format!("{}\n", result)).await;

    runner.poll_workers().await.unwrap();

    assert_task_state(&runner, "task-1", TaskState::Pending, None);
    assert_eq!(
        runner
            .claim_store
            .get(&"task-1".to_string())
            .unwrap()
            .retry_count,
        1
    );
}
