//! Serialization benchmarks: rkyv vs JSON.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::{generate_users, user_to_fields};
use ormdb_core::query::encode_entity;
use ormdb_proto::Value;

fn bench_rkyv_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization/rkyv");

    let users = generate_users(100);

    // Small entity (few fields)
    group.bench_function("serialize_small", |b| {
        let user = &users[0];
        let fields = user_to_fields(user);

        b.iter(|| {
            black_box(encode_entity(&fields).unwrap());
        });
    });

    // Value serialization
    group.bench_function("serialize_string", |b| {
        let value = Value::String("Hello, World! This is a test string.".to_string());

        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&value)).unwrap();
            black_box(bytes);
        });
    });

    group.bench_function("serialize_int64", |b| {
        let value = Value::Int64(1234567890123456789);

        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&value)).unwrap();
            black_box(bytes);
        });
    });

    group.bench_function("serialize_uuid", |b| {
        let value = Value::Uuid([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);

        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&value)).unwrap();
            black_box(bytes);
        });
    });

    group.finish();
}

fn bench_rkyv_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization/rkyv");

    group.bench_function("deserialize_string", |b| {
        let value = Value::String("Hello, World! This is a test string.".to_string());
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).unwrap();

        b.iter(|| {
            let archived =
                rkyv::access::<ormdb_proto::value::ArchivedValue, rkyv::rancor::Error>(
                    black_box(&bytes),
                )
                .unwrap();
            let deserialized: Value =
                rkyv::deserialize::<Value, rkyv::rancor::Error>(archived).unwrap();
            black_box(deserialized);
        });
    });

    group.bench_function("zero_copy_access", |b| {
        let value = Value::String("Hello, World! This is a test string.".to_string());
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).unwrap();

        b.iter(|| {
            let archived =
                rkyv::access::<ormdb_proto::value::ArchivedValue, rkyv::rancor::Error>(
                    black_box(&bytes),
                )
                .unwrap();
            // Just access, don't deserialize
            black_box(archived);
        });
    });

    group.finish();
}

fn bench_json_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization/json");

    let users = generate_users(100);

    group.bench_function("serialize_entity", |b| {
        let user = &users[0];
        let fields = user_to_fields(user);

        // Convert to a serializable format
        let map: std::collections::HashMap<_, _> = fields
            .iter()
            .map(|(k, v)| {
                let json_val = match v {
                    Value::String(s) => serde_json::Value::String(s.clone()),
                    Value::Int32(i) => serde_json::Value::Number((*i).into()),
                    Value::Int64(i) => serde_json::Value::Number((*i).into()),
                    Value::Bool(b) => serde_json::Value::Bool(*b),
                    Value::Uuid(u) => serde_json::Value::String(format!("{:?}", u)),
                    _ => serde_json::Value::Null,
                };
                (k.clone(), json_val)
            })
            .collect();

        b.iter(|| {
            black_box(serde_json::to_vec(&map).unwrap());
        });
    });

    group.bench_function("serialize_string", |b| {
        let value = "Hello, World! This is a test string.";

        b.iter(|| {
            black_box(serde_json::to_vec(black_box(&value)).unwrap());
        });
    });

    group.finish();
}

fn bench_json_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization/json");

    group.bench_function("deserialize_entity", |b| {
        let json = r#"{"name":"Alice_0","email":"user0@example0.com","age":42,"status":"active"}"#;
        let bytes = json.as_bytes();

        b.iter(|| {
            let map: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_slice(black_box(bytes)).unwrap();
            black_box(map);
        });
    });

    group.bench_function("deserialize_string", |b| {
        let json = r#""Hello, World! This is a test string.""#;
        let bytes = json.as_bytes();

        b.iter(|| {
            let value: String = serde_json::from_slice(black_box(bytes)).unwrap();
            black_box(value);
        });
    });

    group.finish();
}

fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization/comparison");

    let users = generate_users(100);
    let user = &users[0];
    let fields = user_to_fields(user);

    // Prepare data for both formats
    let map: std::collections::HashMap<_, _> = fields
        .iter()
        .map(|(k, v)| {
            let json_val = match v {
                Value::String(s) => serde_json::Value::String(s.clone()),
                Value::Int32(i) => serde_json::Value::Number((*i).into()),
                Value::Int64(i) => serde_json::Value::Number((*i).into()),
                Value::Bool(b) => serde_json::Value::Bool(*b),
                Value::Uuid(u) => serde_json::Value::String(format!("{:?}", u)),
                _ => serde_json::Value::Null,
            };
            (k.clone(), json_val)
        })
        .collect();

    let rkyv_bytes = encode_entity(&fields).unwrap();
    let json_bytes = serde_json::to_vec(&map).unwrap();

    println!("rkyv size: {} bytes", rkyv_bytes.len());
    println!("JSON size: {} bytes", json_bytes.len());

    group.bench_function("rkyv_roundtrip", |b| {
        b.iter(|| {
            let bytes = encode_entity(black_box(&fields)).unwrap();
            black_box(bytes);
        });
    });

    group.bench_function("json_roundtrip", |b| {
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&map)).unwrap();
            black_box(bytes);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_rkyv_serialize,
    bench_rkyv_deserialize,
    bench_json_serialize,
    bench_json_deserialize,
    bench_comparison,
);

criterion_main!(benches);
