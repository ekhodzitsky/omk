use criterion::{criterion_group, criterion_main, Criterion};
use omk::runtime::goal::{GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};
use std::path::PathBuf;

fn make_large_task_graph(n: usize) -> GoalTaskGraph {
    let mut tasks = Vec::with_capacity(n);
    for i in 0..n {
        let id = format!("task-{i}");
        let deps: Vec<String> = if i > 0 {
            vec![format!("task-{}", i - 1)]
        } else {
            vec![]
        };
        tasks.push(GoalTask {
            id,
            title: format!("Task {i}"),
            description: format!("Description for task {i}"),
            status: GoalTaskStatus::Pending,
            owner_role: Some("executor".to_string()),
            completed_at: None,
            evidence: vec![GoalTaskEvidence {
                kind: "commit".to_string(),
                path: PathBuf::from(format!("src/task{i}.rs")),
                summary: format!("Evidence for task {i}"),
            }],
            retry_count: 0,
            max_retries: 3,
            lease_expires_at: None,
            dependencies: deps,
            read_set: vec![format!("src/task{i}.rs")],
            write_set: vec![format!("src/task{i}.rs")],
            risk: "low".to_string(),
            acceptance: vec!["Pass gates".to_string()],
        });
    }
    GoalTaskGraph {
        version: 1,
        goal_id: "bench-goal".to_string(),
        generated_at: chrono::Utc::now(),
        tasks,
    }
}

fn bench_goal_task_graph_save(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let graph = make_large_task_graph(1000);

    c.bench_function("goal_task_graph_save_1k", |b| {
        b.to_async(&rt).iter(|| async {
            let path = tmp.path().join("task-graph.json");
            let json = serde_json::to_vec_pretty(&graph).unwrap();
            tokio::fs::write(&path, json).await.unwrap();
        });
    });
}

fn bench_goal_task_graph_load(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let graph = make_large_task_graph(1000);
    rt.block_on(async {
        let path = tmp.path().join("task-graph.json");
        let json = serde_json::to_vec_pretty(&graph).unwrap();
        tokio::fs::write(&path, json).await.unwrap();
    });

    c.bench_function("goal_task_graph_load_1k", |b| {
        b.to_async(&rt).iter(|| async {
            let path = tmp.path().join("task-graph.json");
            let json = tokio::fs::read_to_string(&path).await.unwrap();
            let _graph: GoalTaskGraph = serde_json::from_str(&json).unwrap();
        });
    });
}

fn bench_goal_task_graph_validate(c: &mut Criterion) {
    let graph = make_large_task_graph(1000);

    c.bench_function("goal_task_graph_validate_1k", |b| {
        b.iter(|| {
            graph.validate().unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_goal_task_graph_save,
    bench_goal_task_graph_load,
    bench_goal_task_graph_validate
);
criterion_main!(benches);
