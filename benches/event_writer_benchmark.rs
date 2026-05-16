use criterion::{criterion_group, criterion_main, Criterion};
use omk::runtime::events::{Event, EventBuilder, EventWriter, RunId};

fn make_events(n: usize) -> Vec<Event> {
    (0..n)
        .map(|i| {
            EventBuilder::new(RunId("bench-run".to_string()))
                .run_started("bench", std::path::Path::new("/tmp"), &format!("task {i}"))
                .unwrap()
        })
        .collect()
}

fn bench_event_writer_append_many_1k(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let events = make_events(1000);

    c.bench_function("event_writer_append_many_1k", |b| {
        b.to_async(&rt).iter(|| async {
            let path = tmp.path().join("events.jsonl");
            let writer = EventWriter::new(&path);
            writer.append_many(&events).await.unwrap();
        });
    });
}

fn bench_event_writer_append_1k(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let events = make_events(1000);

    c.bench_function("event_writer_append_1k", |b| {
        b.to_async(&rt).iter(|| async {
            let path = tmp.path().join("events.jsonl");
            let writer = EventWriter::new(&path);
            for event in &events {
                writer.append(event).await.unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_event_writer_append_many_1k,
    bench_event_writer_append_1k
);
criterion_main!(benches);
