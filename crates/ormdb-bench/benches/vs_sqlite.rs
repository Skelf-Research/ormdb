//! SQLite comparison benchmarks.
//!
//! Compares ORMDB against SQLite for equivalent operations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::backends::{OrmdbBackend, SqliteBackend};
use ormdb_bench::fixtures::{generate_users, Scale};

fn bench_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/scan");

    for &scale in &[Scale::Small, Scale::Medium] {
        let name = format!("{:?}", scale);

        // Setup backends
        let ormdb = OrmdbBackend::new(scale);
        let sqlite = SqliteBackend::with_scale(scale);

        // Use row-oriented path (skips EntityBlock transposition)
        group.bench_with_input(BenchmarkId::new("ormdb", &name), &(), |b, _| {
            b.iter(|| {
                let users = ormdb.scan_users_rows();
                black_box(users.len());
            });
        });

        group.bench_with_input(BenchmarkId::new("sqlite", &name), &(), |b, _| {
            b.iter(|| {
                let users = sqlite.scan_users();
                black_box(users.len());
            });
        });
    }

    group.finish();
}

fn bench_scan_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/scan_limit");

    let ormdb = OrmdbBackend::new(Scale::Medium);
    let sqlite = SqliteBackend::with_scale(Scale::Medium);

    // Use row-oriented path (skips EntityBlock transposition)
    for limit in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("ormdb", limit), &limit, |b, &limit| {
            b.iter(|| {
                let users = ormdb.scan_users_limit_rows(limit);
                black_box(users.len());
            });
        });

        group.bench_with_input(BenchmarkId::new("sqlite", limit), &limit, |b, &limit| {
            b.iter(|| {
                let users = sqlite.scan_users_limit(limit);
                black_box(users.len());
            });
        });
    }

    group.finish();
}

fn bench_filter_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/filter_eq");

    let ormdb = OrmdbBackend::new(Scale::Medium);
    let sqlite = SqliteBackend::with_scale(Scale::Medium);

    // Use row-oriented path (skips EntityBlock transposition)
    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let users = ormdb.filter_users_by_status_rows("active");
            black_box(users.len());
        });
    });

    group.bench_function("sqlite", |b| {
        b.iter(|| {
            let users = sqlite.filter_users_by_status("active");
            black_box(users.len());
        });
    });

    group.finish();
}

fn bench_filter_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/filter_range");

    let ormdb = OrmdbBackend::new(Scale::Medium);
    let sqlite = SqliteBackend::with_scale(Scale::Medium);

    // Use row-oriented path (skips EntityBlock transposition)
    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let users = ormdb.filter_users_by_age_gt_rows(50);
            black_box(users.len());
        });
    });

    group.bench_function("sqlite", |b| {
        b.iter(|| {
            let users = sqlite.filter_users_by_age_gt(50);
            black_box(users.len());
        });
    });

    group.finish();
}

fn bench_filter_like(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/filter_like");

    let ormdb = OrmdbBackend::new(Scale::Medium);
    let sqlite = SqliteBackend::with_scale(Scale::Medium);

    // Use row-oriented path (skips EntityBlock transposition)
    group.bench_function("ormdb_prefix", |b| {
        b.iter(|| {
            let users = ormdb.filter_users_by_name_like_rows("Alice%");
            black_box(users.len());
        });
    });

    group.bench_function("sqlite_prefix", |b| {
        b.iter(|| {
            let users = sqlite.filter_users_by_name_like("Alice%");
            black_box(users.len());
        });
    });

    group.bench_function("ormdb_contains", |b| {
        b.iter(|| {
            let users = ormdb.filter_users_by_name_like_rows("%_1%");
            black_box(users.len());
        });
    });

    group.bench_function("sqlite_contains", |b| {
        b.iter(|| {
            let users = sqlite.filter_users_by_name_like("%_1%");
            black_box(users.len());
        });
    });

    group.finish();
}

fn bench_n_plus_1(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/n_plus_1");

    // Use Small scale for N+1 to avoid very long benchmark times
    let ormdb = OrmdbBackend::new(Scale::Small);
    let sqlite = SqliteBackend::with_scale(Scale::Small);

    for limit in [10, 50, 100] {
        // ORMDB automatically eliminates N+1
        group.bench_with_input(
            BenchmarkId::new("ormdb_batched", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = ormdb.get_users_with_posts(limit);
                    black_box(results.len());
                });
            },
        );

        // SQLite N+1 pattern (anti-pattern)
        group.bench_with_input(
            BenchmarkId::new("sqlite_n_plus_1", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = sqlite.get_users_with_posts_n_plus_1(limit);
                    black_box(results.len());
                });
            },
        );

        // SQLite batched (similar to ORMDB)
        group.bench_with_input(
            BenchmarkId::new("sqlite_batched", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = sqlite.get_users_with_posts_batched(limit);
                    black_box(results.len());
                });
            },
        );

        // SQLite JOIN (most efficient for SQL)
        group.bench_with_input(
            BenchmarkId::new("sqlite_join", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = sqlite.get_users_with_posts_join(limit);
                    black_box(results.len());
                });
            },
        );
    }

    group.finish();
}

fn bench_insert_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/insert_single");

    // Create fresh backends for mutation benchmarks (empty, just schema)
    let ormdb_ctx = ormdb_bench::harness::TestContext::with_schema();
    let sqlite = SqliteBackend::new();
    sqlite.setup_schema();

    // Generate unique users for insertion - use a large pool
    let users = generate_users(10000);
    let mut ormdb_idx = 0;
    let mut sqlite_idx = 0;

    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let user = &users[ormdb_idx % users.len()];
            ormdb_idx += 1;
            let fields = ormdb_bench::fixtures::user_to_fields(user);
            ormdb_bench::harness::insert_entity(&ormdb_ctx.storage, "User", fields);
        });
    });

    group.bench_function("sqlite", |b| {
        b.iter(|| {
            let user = &users[sqlite_idx % users.len()];
            sqlite_idx += 1;
            sqlite.insert_user(user);
        });
    });

    group.finish();
}

fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_sqlite/insert_batch");

    for batch_size in [10, 100] {
        // Create fresh backends for each batch size
        let ormdb = OrmdbBackend::new(Scale::Small);
        let sqlite = SqliteBackend::with_scale(Scale::Small);

        // Generate unique users for insertion
        let users = generate_users(batch_size * 10);

        group.bench_with_input(
            BenchmarkId::new("ormdb", batch_size),
            &batch_size,
            |b, &batch_size| {
                let mut offset = 0;
                b.iter(|| {
                    let batch = &users[offset..offset + batch_size];
                    offset = (offset + batch_size) % (users.len() - batch_size);
                    ormdb.insert_users_batch(batch);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sqlite", batch_size),
            &batch_size,
            |b, &batch_size| {
                let mut offset = 0;
                b.iter(|| {
                    let batch = &users[offset..offset + batch_size];
                    offset = (offset + batch_size) % (users.len() - batch_size);
                    sqlite.insert_users_batch(batch);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_scan,
    bench_scan_limit,
    bench_filter_eq,
    bench_filter_range,
    bench_filter_like,
    bench_n_plus_1,
    bench_insert_single,
    bench_insert_batch,
);

criterion_main!(benches);
