//! Join strategy benchmarks.
//!
//! Compares NestedLoop vs HashJoin strategies at different cardinalities.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::Scale;
use ormdb_bench::harness::TestContext;
use ormdb_proto::{GraphQuery, Pagination, RelationInclude};

fn bench_nested_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("join/nested_loop");

    // Small parent set - NestedLoop should be efficient
    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    for parent_limit in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("parents", parent_limit),
            &parent_limit,
            |b, &limit| {
                let query = GraphQuery::new("User")
                    .include(RelationInclude::new("posts"))
                    .with_pagination(Pagination::limit(limit as u32));

                b.iter(|| {
                    black_box(executor.execute(&query).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_hash_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("join/hash_join");

    // Larger parent set - HashJoin should be more efficient
    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    for parent_limit in [100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("parents", parent_limit),
            &parent_limit,
            |b, &limit| {
                let query = GraphQuery::new("User")
                    .include(RelationInclude::new("posts"))
                    .with_pagination(Pagination::limit(limit as u32));

                b.iter(|| {
                    black_box(executor.execute(&query).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_crossover_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("join/crossover");

    // Find the optimal threshold
    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    for parent_count in [50, 75, 100, 125, 150, 200] {
        group.bench_with_input(
            BenchmarkId::new("parents", parent_count),
            &parent_count,
            |b, &count| {
                let query = GraphQuery::new("User")
                    .include(RelationInclude::new("posts"))
                    .with_pagination(Pagination::limit(count as u32));

                b.iter(|| {
                    black_box(executor.execute(&query).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_multi_level_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("join/multi_level");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    group.bench_function("2_levels", |b| {
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments"))
            .with_pagination(Pagination::limit(10));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("with_child_limit", |b| {
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts").with_pagination(Pagination::limit(5)))
            .with_pagination(Pagination::limit(10));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_join_with_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("join/filtered");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    group.bench_function("parent_filter", |b| {
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .with_filter(ormdb_proto::FilterExpr::eq("status", ormdb_proto::Value::String("active".to_string())).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("child_filter", |b| {
        let query = GraphQuery::new("User").include(
            RelationInclude::new("posts").with_filter(
                ormdb_proto::FilterExpr::eq("published", ormdb_proto::Value::Bool(true)).into(),
            ),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("both_filters", |b| {
        let query = GraphQuery::new("User")
            .include(
                RelationInclude::new("posts").with_filter(
                    ormdb_proto::FilterExpr::eq("published", ormdb_proto::Value::Bool(true)).into(),
                ),
            )
            .with_filter(ormdb_proto::FilterExpr::eq("status", ormdb_proto::Value::String("active".to_string())).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_nested_loop,
    bench_hash_join,
    bench_crossover_point,
    bench_multi_level_join,
    bench_join_with_filter,
);

criterion_main!(benches);
