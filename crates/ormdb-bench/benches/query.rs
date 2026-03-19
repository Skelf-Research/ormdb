//! Query executor benchmarks.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::Scale;
use ormdb_bench::harness::TestContext;
use ormdb_proto::{FilterExpr, GraphQuery, OrderSpec, Pagination, RelationInclude, Value};

fn bench_simple_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/scan");

    for &scale in &[Scale::Small, Scale::Medium] {
        let ctx = TestContext::with_scale(scale);
        let name = format!("{:?}", scale);

        group.bench_with_input(BenchmarkId::new("simple", &name), &(), |b, _| {
            let executor = ctx.executor();
            let query = GraphQuery::new("User");

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_filtered_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/filter");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    // Selective filter (~1% match)
    group.bench_function("eq_selective", |b| {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String("admin".to_string())).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    // Non-selective filter (~25% match)
    group.bench_function("eq_broad", |b| {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_filtered_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/filter");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("range_narrow", |b| {
        // Match ~5% of data (ages 70-77)
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(70)).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("range_broad", |b| {
        // Match ~50% of data (ages > 48)
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(48)).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_filtered_like(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/filter");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("like_prefix", |b| {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::like("name", "Alice%").into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("like_contains", |b| {
        let query = GraphQuery::new("User").with_filter(FilterExpr::like("name", "%_1%").into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_sorted(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/sort");

    for &scale in &[Scale::Small, Scale::Medium] {
        let ctx = TestContext::with_scale(scale);
        let executor = ctx.executor();
        let name = format!("{:?}", scale);

        group.bench_with_input(BenchmarkId::new("asc", &name), &(), |b, _| {
            let query = GraphQuery::new("User").with_order(OrderSpec::asc("name"));

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("desc", &name), &(), |b, _| {
            let query = GraphQuery::new("User").with_order(OrderSpec::desc("age"));

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_paginated(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/pagination");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    for &(offset, name) in &[(0, "first"), (5000, "middle"), (9900, "last")] {
        group.bench_function(name, |b| {
            let query = GraphQuery::new("User")
                .with_order(OrderSpec::asc("name"))
                .with_pagination(Pagination::new(100, offset));

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_with_include(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/include");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    group.bench_function("single_relation", |b| {
        let query = GraphQuery::new("User").include(RelationInclude::new("posts"));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("nested_relation", |b| {
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments"));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("with_filter", |b| {
        let query = GraphQuery::new("User").include(
            RelationInclude::new("posts")
                .with_filter(FilterExpr::eq("published", Value::Bool(true)).into()),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_scan,
    bench_filtered_eq,
    bench_filtered_range,
    bench_filtered_like,
    bench_sorted,
    bench_paginated,
    bench_with_include,
);

criterion_main!(benches);
