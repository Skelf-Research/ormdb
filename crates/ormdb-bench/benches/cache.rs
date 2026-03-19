//! Plan cache benchmarks.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::Scale;
use ormdb_bench::harness::TestContext;
use ormdb_proto::{FilterExpr, GraphQuery, OrderSpec, Pagination, RelationInclude, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Compute a fingerprint for a query (simulating plan cache key).
fn compute_fingerprint(query: &GraphQuery) -> u64 {
    let mut hasher = DefaultHasher::new();
    query.root_entity.hash(&mut hasher);
    query.fields.hash(&mut hasher);
    // Note: Filter fingerprinting would need a custom implementation
    // This is a simplified version for benchmarking
    hasher.finish()
}

fn bench_fingerprint_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache/fingerprint");

    group.bench_function("simple_query", |b| {
        let query = GraphQuery::new("User");

        b.iter(|| {
            black_box(compute_fingerprint(&query));
        });
    });

    group.bench_function("with_fields", |b| {
        let query = GraphQuery::new("User").with_fields(vec![
            "id".into(),
            "name".into(),
            "email".into(),
            "age".into(),
            "status".into(),
        ]);

        b.iter(|| {
            black_box(compute_fingerprint(&query));
        });
    });

    group.bench_function("complex_query", |b| {
        let query = GraphQuery::new("User")
            .with_fields(vec!["id".into(), "name".into()])
            .include(RelationInclude::new("posts"))
            .with_order(OrderSpec::asc("name"))
            .with_pagination(Pagination::new(10, 0));

        b.iter(|| {
            black_box(compute_fingerprint(&query));
        });
    });

    group.finish();
}

fn bench_cache_hit_vs_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache/execution");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    // First execution (cold/planning)
    group.bench_function("cold_execution", |b| {
        b.iter_batched(
            || {
                // Create a new query each time to prevent caching benefits
                GraphQuery::new("User")
                    .with_filter(FilterExpr::gt("age", Value::Int32(rand::random::<i32>() % 50)).into())
            },
            |query| {
                black_box(executor.execute(&query).unwrap());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Repeated execution (should benefit from any caching)
    group.bench_function("warm_execution", |b| {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::gt("age", Value::Int32(30)).into());

        // Warm up
        let _ = executor.execute(&query);

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_planning_vs_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache/ratio");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    // Measure just the query execution (includes planning)
    for complexity in ["simple", "filtered", "with_include"] {
        let query = match complexity {
            "simple" => GraphQuery::new("User"),
            "filtered" => {
                GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(30)).into())
            }
            "with_include" => GraphQuery::new("User").include(RelationInclude::new("posts")),
            _ => unreachable!(),
        };

        group.bench_with_input(BenchmarkId::new("total", complexity), &query, |b, query| {
            b.iter(|| {
                black_box(executor.execute(query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_query_variation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache/variation");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    // Same structure, different parameter values (cache should still help)
    group.bench_function("same_structure_diff_params", |b| {
        let mut age = 20;

        b.iter(|| {
            age = (age + 1) % 60 + 18;
            let query =
                GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(age)).into());
            black_box(executor.execute(&query).unwrap());
        });
    });

    // Different query structures (cache may not help)
    group.bench_function("different_structures", |b| {
        let queries = vec![
            GraphQuery::new("User"),
            GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(30)).into()),
            GraphQuery::new("User").with_order(OrderSpec::asc("name")),
            GraphQuery::new("User").include(RelationInclude::new("posts")),
        ];
        let mut idx = 0;

        b.iter(|| {
            let query = &queries[idx % queries.len()];
            idx += 1;
            black_box(executor.execute(query).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_fingerprint_compute,
    bench_cache_hit_vs_miss,
    bench_planning_vs_execution,
    bench_query_variation,
);

criterion_main!(benches);
