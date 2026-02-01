//! PostgreSQL comparison benchmarks.
//!
//! Requires:
//! - `--features postgres` flag
//! - `DATABASE_URL` environment variable
//!
//! Example: DATABASE_URL=postgres://localhost/ormdb_bench cargo bench --bench vs_postgres --features postgres

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::backends::{OrmdbBackend, PostgresBackend};
use ormdb_bench::fixtures::{generate_users, Scale};

fn bench_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/scan");

    // Use Small scale for PostgreSQL benchmarks (faster setup)
    let ormdb = OrmdbBackend::new(Scale::Small);
    let postgres = PostgresBackend::with_scale(Scale::Small);

    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let users = ormdb.scan_users();
            black_box(users.len());
        });
    });

    group.bench_function("postgres", |b| {
        b.iter(|| {
            let users = postgres.scan_users();
            black_box(users.len());
        });
    });

    group.finish();
}

fn bench_scan_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/scan_limit");

    let ormdb = OrmdbBackend::new(Scale::Small);
    let postgres = PostgresBackend::with_scale(Scale::Small);

    for limit in [10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("ormdb", limit), &limit, |b, &limit| {
            b.iter(|| {
                let users = ormdb.scan_users_limit(limit);
                black_box(users.len());
            });
        });

        group.bench_with_input(BenchmarkId::new("postgres", limit), &limit, |b, &limit| {
            b.iter(|| {
                let users = postgres.scan_users_limit(limit);
                black_box(users.len());
            });
        });
    }

    group.finish();
}

fn bench_filter_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/filter_eq");

    let ormdb = OrmdbBackend::new(Scale::Small);
    let postgres = PostgresBackend::with_scale(Scale::Small);

    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let users = ormdb.filter_users_by_status("active");
            black_box(users.len());
        });
    });

    group.bench_function("postgres", |b| {
        b.iter(|| {
            let users = postgres.filter_users_by_status("active");
            black_box(users.len());
        });
    });

    group.finish();
}

fn bench_filter_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/filter_range");

    let ormdb = OrmdbBackend::new(Scale::Small);
    let postgres = PostgresBackend::with_scale(Scale::Small);

    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let users = ormdb.filter_users_by_age_gt(50);
            black_box(users.len());
        });
    });

    group.bench_function("postgres", |b| {
        b.iter(|| {
            let users = postgres.filter_users_by_age_gt(50);
            black_box(users.len());
        });
    });

    group.finish();
}

fn bench_n_plus_1(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/n_plus_1");

    let ormdb = OrmdbBackend::new(Scale::Small);
    let postgres = PostgresBackend::with_scale(Scale::Small);

    for limit in [10, 50] {
        // ORMDB automatic batching
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

        // PostgreSQL N+1 (anti-pattern)
        group.bench_with_input(
            BenchmarkId::new("postgres_n_plus_1", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = postgres.get_users_with_posts_n_plus_1(limit);
                    black_box(results.len());
                });
            },
        );

        // PostgreSQL batched
        group.bench_with_input(
            BenchmarkId::new("postgres_batched", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = postgres.get_users_with_posts_batched(limit);
                    black_box(results.len());
                });
            },
        );

        // PostgreSQL JOIN
        group.bench_with_input(
            BenchmarkId::new("postgres_join", limit),
            &limit,
            |b, &limit| {
                b.iter(|| {
                    let results = postgres.get_users_with_posts_join(limit);
                    black_box(results.len());
                });
            },
        );
    }

    group.finish();
}

fn bench_insert_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/insert_single");

    let ormdb = OrmdbBackend::new(Scale::Small);
    let postgres = PostgresBackend::with_scale(Scale::Small);

    let users = generate_users(500);
    let mut user_idx = 100;

    group.bench_function("ormdb", |b| {
        b.iter(|| {
            let user = &users[user_idx % users.len()];
            user_idx += 1;
            ormdb.insert_user(user);
        });
    });

    user_idx = 100;
    group.bench_function("postgres", |b| {
        b.iter(|| {
            let user = &users[user_idx % users.len()];
            user_idx += 1;
            postgres.insert_user(user);
        });
    });

    group.finish();
}

fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("vs_postgres/insert_batch");

    for batch_size in [10, 50] {
        let ormdb = OrmdbBackend::new(Scale::Small);
        let postgres = PostgresBackend::with_scale(Scale::Small);

        let users = generate_users(batch_size * 5);

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
            BenchmarkId::new("postgres", batch_size),
            &batch_size,
            |b, &batch_size| {
                let mut offset = 0;
                b.iter(|| {
                    let batch = &users[offset..offset + batch_size];
                    offset = (offset + batch_size) % (users.len() - batch_size);
                    postgres.insert_users_batch(batch);
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
    bench_n_plus_1,
    bench_insert_single,
    bench_insert_batch,
);

criterion_main!(benches);
