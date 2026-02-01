//! Storage engine benchmarks.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::harness::{setup_raw_storage, TestContext};
use ormdb_bench::fixtures::{generate_record_data, Scale};
use ormdb_core::storage::{Record, StorageEngine, VersionedKey};

fn bench_put_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/put");

    for size in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("single", size), &size, |b, &size| {
            let (storage, _dir) = setup_raw_storage();
            let data = generate_record_data(size);

            b.iter(|| {
                let id = StorageEngine::generate_id();
                let key = VersionedKey::now(id);
                storage.put(key, Record::new(black_box(data.clone()))).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_put_typed(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/put_typed");

    for size in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("single", size), &size, |b, &size| {
            let ctx = TestContext::with_schema();
            let data = generate_record_data(size);

            b.iter(|| {
                let id = StorageEngine::generate_id();
                let key = VersionedKey::now(id);
                ctx.storage
                    .put_typed("User", key, Record::new(black_box(data.clone())))
                    .unwrap();
            });
        });
    }

    group.finish();
}

fn bench_get_latest(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/get");

    for &scale in &[Scale::Small, Scale::Medium] {
        let ctx = TestContext::with_scale(scale);

        // Collect some IDs for lookup
        let ids: Vec<_> = ctx
            .storage
            .scan_entity_type("User")
            .take(100)
            .filter_map(|r| r.ok())
            .map(|(id, _, _)| id)
            .collect();

        let name = format!("{:?}", scale);
        group.bench_with_input(BenchmarkId::new("latest", &name), &ids, |b, ids| {
            let mut idx = 0;
            b.iter(|| {
                let id = ids[idx % ids.len()];
                idx += 1;
                black_box(ctx.storage.get_latest(&id).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_scan_entity_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/scan");

    for &scale in &[Scale::Small, Scale::Medium] {
        let ctx = TestContext::with_scale(scale);
        let name = format!("{:?}", scale);

        group.bench_with_input(
            BenchmarkId::new("entity_type", &name),
            &(),
            |b, _| {
                b.iter(|| {
                    let count = ctx
                        .storage
                        .scan_entity_type("User")
                        .count();
                    black_box(count);
                });
            },
        );
    }

    group.finish();
}

fn bench_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage/transaction");

    for op_count in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("commit", op_count),
            &op_count,
            |b, &op_count| {
                let ctx = TestContext::with_schema();
                let data = generate_record_data(100);

                b.iter(|| {
                    let mut txn = ctx.storage.transaction();
                    for _ in 0..op_count {
                        let id = StorageEngine::generate_id();
                        let key = VersionedKey::now(id);
                        txn.put_typed("User", key, Record::new(data.clone()));
                    }
                    txn.commit().unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_put_single,
    bench_put_typed,
    bench_get_latest,
    bench_scan_entity_type,
    bench_transaction,
);

criterion_main!(benches);
