use criterion::{criterion_group, criterion_main, Criterion};
use omk::wire::redact_wire_secrets;
use serde_json::json;

fn make_deep_json(depth: usize) -> serde_json::Value {
    let mut value = json!({"secret": "ghp_1234567890abcdef1234567890abcdef1234"});
    for _ in 0..depth {
        value = json!({"nested": value});
    }
    value
}

fn bench_redact_wire_secrets_shallow(c: &mut Criterion) {
    let value = make_deep_json(5);
    c.bench_function("redact_wire_secrets_depth_5", |b| {
        b.iter(|| {
            let _ = redact_wire_secrets(&value);
        });
    });
}

fn bench_redact_wire_secrets_deep(c: &mut Criterion) {
    let value = make_deep_json(50);
    c.bench_function("redact_wire_secrets_depth_50", |b| {
        b.iter(|| {
            let _ = redact_wire_secrets(&value);
        });
    });
}

fn bench_redact_wire_secrets_wide(c: &mut Criterion) {
    let mut obj = serde_json::Map::new();
    for i in 0..1000 {
        obj.insert(format!("key_{i}"), json!(format!("value_{i}")));
    }
    obj.insert("api_key".to_string(), json!("sk_live_1234567890abcdef"));
    let value = serde_json::Value::Object(obj);

    c.bench_function("redact_wire_secrets_wide_1k", |b| {
        b.iter(|| {
            let _ = redact_wire_secrets(&value);
        });
    });
}

criterion_group!(
    benches,
    bench_redact_wire_secrets_shallow,
    bench_redact_wire_secrets_deep,
    bench_redact_wire_secrets_wide
);
criterion_main!(benches);
