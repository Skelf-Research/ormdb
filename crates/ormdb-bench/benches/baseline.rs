//! Internal baseline benchmarks.
//!
//! Compares raw storage operations vs typed operations to measure
//! the overhead of type indexing and entity management.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::{generate_record_data, generate_users, user_to_fields, Scale};
use ormdb_bench::harness::{setup_raw_storage, TestContext};
use ormdb_core::query::encode_entity;
use ormdb_core::storage::{Record, VersionedKey};

fn bench_raw_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/put");

    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("raw", size), &size, |b, &size| {
            let (storage, _dir) = setup_raw_storage();
            let data = generate_record_data(size);

            b.iter(|| {
                let id = ormdb_core::storage::StorageEngine::generate_id();
                let key = VersionedKey::now(id);
                // Create record inside iter to avoid measuring clone overhead
                let record = Record::new(data.clone());
                black_box(storage.put(key, record).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("typed", size), &size, |b, &size| {
            let ctx = TestContext::with_schema();
            let data = generate_record_data(size);

            b.iter(|| {
                let id = ormdb_core::storage::StorageEngine::generate_id();
                let key = VersionedKey::now(id);
                // Create record inside iter to avoid measuring clone overhead
                let record = Record::new(data.clone());
                black_box(ctx.storage.put_typed("User", key, record).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_raw_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/get");

    // Setup: pre-populate with 1000 records
    let (storage, _dir) = setup_raw_storage();
    let mut ids = Vec::with_capacity(1000);
    let data = generate_record_data(500);
    let record = Record::new(data);

    for _ in 0..1000 {
        let id = ormdb_core::storage::StorageEngine::generate_id();
        let key = VersionedKey::now(id);
        storage.put(key, record.clone()).unwrap();
        ids.push(id);
    }
    storage.flush().unwrap();

    group.bench_function("raw_get_latest", |b| {
        let mut idx = 0;
        b.iter(|| {
            let id = ids[idx % ids.len()];
            idx += 1;
            black_box(storage.get_latest(&id).unwrap());
        });
    });

    group.finish();
}

fn bench_typed_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/scan");

    for &scale in &[Scale::Small, Scale::Medium] {
        let ctx = TestContext::with_scale(scale);
        let name = format!("{:?}", scale);

        group.bench_with_input(BenchmarkId::new("typed_scan", &name), &(), |b, _| {
            b.iter(|| {
                let entities: Vec<_> = ctx.storage.scan_entity_type("User").collect();
                black_box(entities.len());
            });
        });
    }

    group.finish();
}

fn bench_encode_entity(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/encode");

    let users = generate_users(10);

    group.bench_function("user_to_fields", |b| {
        b.iter(|| {
            let fields = user_to_fields(&users[0]);
            black_box(fields);
        });
    });

    group.bench_function("encode_entity", |b| {
        let fields = user_to_fields(&users[0]);
        b.iter(|| {
            let encoded = encode_entity(&fields).unwrap();
            black_box(encoded);
        });
    });

    group.finish();
}

fn bench_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/transaction");

    for count in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("commit", count), &count, |b, &count| {
            let ctx = TestContext::with_schema();
            let users = generate_users(count);

            b.iter(|| {
                let mut txn = ctx.storage.transaction();
                for user in &users {
                    let fields = user_to_fields(user);
                    let data = encode_entity(&fields).unwrap();
                    let key = VersionedKey::now(user.id);
                    txn.put_typed("User", key, Record::new(data));
                }
                txn.commit().unwrap();
            });
        });
    }

    group.finish();
}

fn bench_version_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/versions");

    // Create an entity with multiple versions
    let ctx = TestContext::with_schema();
    let user = &generate_users(1)[0];
    let id = user.id;

    // Insert 10 versions with minimal sleep (just enough for unique timestamps)
    for i in 0..10 {
        let mut fields = user_to_fields(user);
        fields[1] = ("name".to_string(), ormdb_proto::Value::String(format!("Name_v{}", i)));
        let data = encode_entity(&fields).unwrap();
        let key = VersionedKey::now(id);
        ctx.storage.put_typed("User", key, Record::new(data)).unwrap();
        // Use microseconds instead of milliseconds to reduce setup overhead
        // while still ensuring unique timestamps
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
    ctx.storage.flush().unwrap();

    group.bench_function("get_latest", |b| {
        b.iter(|| {
            black_box(ctx.storage.get_latest(&id).unwrap());
        });
    });

    group.bench_function("scan_versions", |b| {
        b.iter(|| {
            let versions: Vec<_> = ctx.storage.scan_versions(&id).collect();
            black_box(versions.len());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_raw_put,
    bench_raw_get,
    bench_typed_scan,
    bench_encode_entity,
    bench_transaction,
    bench_version_overhead,
);

criterion_main!(benches);
