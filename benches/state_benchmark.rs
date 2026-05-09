use criterion::{criterion_group, criterion_main, Criterion};
use omk::runtime::state::TeamState;

fn bench_team_state_save(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    c.bench_function("team_state_save", |b| {
        b.to_async(&rt).iter(|| async {
            let state = TeamState::new("bench", "benchmark task", tmp.path(), 4, "coder");
            state.save().await.unwrap();
        });
    });
}

fn bench_team_state_load(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    rt.block_on(async {
        let state = TeamState::new("bench", "benchmark task", tmp.path(), 4, "coder");
        state.save().await.unwrap();
    });

    c.bench_function("team_state_load", |b| {
        b.to_async(&rt).iter(|| async {
            let _state = TeamState::load(tmp.path()).await.unwrap();
        });
    });
}

fn bench_atomic_write(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("test.json");
    let content = vec![0u8; 1024 * 1024]; // 1 MB

    c.bench_function("atomic_write_1mb", |b| {
        b.to_async(&rt).iter(|| async {
            omk::runtime::atomic::atomic_write(&path, &content)
                .await
                .unwrap();
        });
    });
}

fn bench_shell_escape(c: &mut Criterion) {
    c.bench_function("shell_escape_simple", |b| {
        b.iter(|| omk::runtime::shell::shell_escape("hello world"))
    });

    c.bench_function("shell_escape_complex", |b| {
        b.iter(|| omk::runtime::shell::shell_escape("it's a $test `command`"))
    });
}

fn bench_validate_safe(c: &mut Criterion) {
    c.bench_function("validate_safe_ok", |b| {
        b.iter(|| omk::runtime::shell::validate_safe("hello world 123"))
    });

    c.bench_function("validate_safe_null", |b| {
        b.iter(|| omk::runtime::shell::validate_safe("hello\0world"))
    });
}

criterion_group!(
    benches,
    bench_team_state_save,
    bench_team_state_load,
    bench_atomic_write,
    bench_shell_escape,
    bench_validate_safe
);
criterion_main!(benches);
