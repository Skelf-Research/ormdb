//! End-to-end benchmarks.
//!
//! These benchmarks measure latency through the full stack.
//! Note: Requires running a server instance for client benchmarks.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::Scale;
use ormdb_bench::harness::TestContext;
use ormdb_proto::{FilterExpr, GraphQuery, OrderSpec, Pagination, RelationInclude, Value};

// In-process benchmarks (no network)
fn bench_in_process_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e/in_process");

    for &scale in &[Scale::Small, Scale::Medium] {
        let ctx = TestContext::with_scale(scale);
        let executor = ctx.executor();
        let name = format!("{:?}", scale);

        group.bench_with_input(BenchmarkId::new("simple_query", &name), &(), |b, _| {
            let query = GraphQuery::new("User");

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("filtered_query", &name), &(), |b, _| {
            let query = GraphQuery::new("User")
                .with_filter(FilterExpr::gt("age", Value::Int32(30)).into())
                .with_order(OrderSpec::asc("name"))
                .with_pagination(Pagination::new(100, 0));

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("graph_query", &name), &(), |b, _| {
            let query = GraphQuery::new("User")
                .include(RelationInclude::new("posts"))
                .with_pagination(Pagination::limit(50));

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e/workload");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    // Simulate a typical read-heavy workload
    group.bench_function("read_heavy_mix", |b| {
        let queries = vec![
            // Simple lookup (40%)
            GraphQuery::new("User")
                .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into())
                .with_pagination(Pagination::limit(10)),
            GraphQuery::new("User")
                .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into())
                .with_pagination(Pagination::limit(10)),
            GraphQuery::new("User")
                .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into())
                .with_pagination(Pagination::limit(10)),
            GraphQuery::new("User")
                .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into())
                .with_pagination(Pagination::limit(10)),
            // Sorted list (20%)
            GraphQuery::new("User")
                .with_order(OrderSpec::desc("age"))
                .with_pagination(Pagination::new(20, 0)),
            GraphQuery::new("User")
                .with_order(OrderSpec::desc("age"))
                .with_pagination(Pagination::new(20, 0)),
            // Graph query (20%)
            GraphQuery::new("User")
                .include(RelationInclude::new("posts").with_pagination(Pagination::limit(5)))
                .with_pagination(Pagination::limit(10)),
            GraphQuery::new("User")
                .include(RelationInclude::new("posts").with_pagination(Pagination::limit(5)))
                .with_pagination(Pagination::limit(10)),
            // Complex filter (10%)
            GraphQuery::new("User").with_filter(
                FilterExpr::and(vec![
                    ormdb_proto::SimpleFilter::Gt {
                        field: "age".to_string(),
                        value: Value::Int32(25),
                    },
                    ormdb_proto::SimpleFilter::Like {
                        field: "email".to_string(),
                        pattern: "%example%".to_string(),
                    },
                ])
                .into(),
            ),
            // Full scan (10%)
            GraphQuery::new("Post").with_pagination(Pagination::limit(100)),
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

fn bench_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e/latency");

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    // Simple query for p50 baseline
    group.bench_function("p50_simple", |b| {
        let query = GraphQuery::new("User").with_pagination(Pagination::limit(10));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    // Complex query for p95/p99 scenarios
    group.bench_function("p99_complex", |b| {
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("posts.comments"))
            .with_filter(FilterExpr::gt("age", Value::Int32(25)).into())
            .with_order(OrderSpec::asc("name"))
            .with_pagination(Pagination::limit(50));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e/throughput");
    group.throughput(criterion::Throughput::Elements(1));

    let ctx = TestContext::with_scale(Scale::Small);
    let executor = ctx.executor();

    group.bench_function("queries_per_second", |b| {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into())
            .with_pagination(Pagination::limit(10));

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_in_process_query,
    bench_realistic_workload,
    bench_latency_percentiles,
    bench_throughput,
);

criterion_main!(benches);
