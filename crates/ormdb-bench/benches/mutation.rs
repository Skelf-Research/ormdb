//! Mutation benchmarks.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::{generate_users, user_to_fields, Scale};
use ormdb_bench::harness::{insert_entity, TestContext};
use ormdb_core::storage::{StorageEngine, VersionedKey, Record};
use ormdb_core::query::encode_entity;
use ormdb_proto::Value;

fn bench_insert_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutation/insert");

    let users = generate_users(1000);

    group.bench_function("single_small", |b| {
        let ctx = TestContext::with_schema();
        let mut idx = 0;

        b.iter(|| {
            let user = &users[idx % users.len()];
            idx += 1;

            let mut fields = user_to_fields(user);
            // Use a new ID each time
            let id = StorageEngine::generate_id();
            fields[0] = ("id".to_string(), Value::Uuid(id));

            insert_entity(&ctx.storage, "User", fields);
        });
    });

    group.bench_function("single_with_flush", |b| {
        let ctx = TestContext::with_schema();
        let mut idx = 0;

        b.iter(|| {
            let user = &users[idx % users.len()];
            idx += 1;

            let mut fields = user_to_fields(user);
            let id = StorageEngine::generate_id();
            fields[0] = ("id".to_string(), Value::Uuid(id));

            insert_entity(&ctx.storage, "User", fields);
            ctx.storage.flush().unwrap();
        });
    });

    group.finish();
}

fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutation/insert_batch");

    let users = generate_users(1000);

    for batch_size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                let ctx = TestContext::with_schema();

                b.iter(|| {
                    let mut txn = ctx.storage.transaction();

                    for i in 0..batch_size {
                        let user = &users[i % users.len()];
                        let mut fields = user_to_fields(user);
                        let id = StorageEngine::generate_id();
                        fields[0] = ("id".to_string(), Value::Uuid(id));

                        let data = encode_entity(&fields).unwrap();
                        let key = VersionedKey::now(id);
                        txn.put_typed("User", key, Record::new(data));
                    }

                    txn.commit().unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_update_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutation/update");

    let ctx = TestContext::with_scale(Scale::Small);

    // Get some existing IDs - scan_entity_type returns Iterator<Item = Result<([u8; 16], u64, Record), Error>>
    let ids: Vec<_> = ctx
        .storage
        .scan_entity_type("User")
        .take(100)
        .filter_map(|r| r.ok())
        .map(|(id, _, _)| id)
        .collect();

    group.bench_function("single_field", |b| {
        let mut idx = 0;

        b.iter(|| {
            let id = ids[idx % ids.len()];
            idx += 1;

            // Read existing, modify, write back
            let (_key, record) = ctx.storage.get_latest(&id).unwrap().unwrap();
            let mut data = record.data.clone();
            // Simulate modification
            if !data.is_empty() {
                data[0] = data[0].wrapping_add(1);
            }

            let key = VersionedKey::now(id);
            ctx.storage.put_typed("User", key, Record::new(data)).unwrap();
        });
    });

    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutation/delete");

    group.bench_function("soft_delete", |b| {
        let ctx = TestContext::with_scale(Scale::Small);

        let ids: Vec<_> = ctx
            .storage
            .scan_entity_type("User")
            .take(100)
            .filter_map(|r| r.ok())
            .map(|(id, _, _)| id)
            .collect();

        let mut idx = 0;

        b.iter(|| {
            let id = ids[idx % ids.len()];
            idx += 1;

            // Soft delete: write tombstone
            let key = VersionedKey::now(id);
            black_box(ctx.storage.put_typed("User", key, Record::tombstone()).unwrap());
        });
    });

    group.finish();
}

fn bench_upsert(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutation/upsert");

    let users = generate_users(100);
    let ctx = TestContext::with_scale(Scale::Small);

    // Get existing IDs
    let existing_ids: Vec<_> = ctx
        .storage
        .scan_entity_type("User")
        .take(50)
        .filter_map(|r| r.ok())
        .map(|(id, _, _)| id)
        .collect();

    group.bench_function("insert_path", |b| {
        let mut idx = 0;

        b.iter(|| {
            let user = &users[idx % users.len()];
            idx += 1;

            let mut fields = user_to_fields(user);
            let id = StorageEngine::generate_id();
            fields[0] = ("id".to_string(), Value::Uuid(id));

            // Upsert with new ID (insert path)
            insert_entity(&ctx.storage, "User", fields);
        });
    });

    group.bench_function("update_path", |b| {
        let mut idx = 0;

        b.iter(|| {
            let id = existing_ids[idx % existing_ids.len()];
            idx += 1;

            let user = &users[idx % users.len()];
            let mut fields = user_to_fields(user);
            fields[0] = ("id".to_string(), Value::Uuid(id));

            // Upsert with existing ID (update path)
            insert_entity(&ctx.storage, "User", fields);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_single,
    bench_insert_batch,
    bench_update_single,
    bench_delete,
    bench_upsert,
);

criterion_main!(benches);
