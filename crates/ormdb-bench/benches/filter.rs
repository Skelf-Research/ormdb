//! Filter evaluation benchmarks.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ormdb_bench::fixtures::Scale;
use ormdb_bench::harness::TestContext;
use ormdb_proto::{FilterExpr, GraphQuery, SimpleFilter, Value};

fn bench_eval_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/eq");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("string", |b| {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("name", Value::String("Alice_0".to_string())).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("int32", |b| {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::eq("age", Value::Int32(30)).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_eval_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/range");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("gt", |b| {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(50)).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("lt", |b| {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::lt("age", Value::Int32(30)).into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("between", |b| {
        let query = GraphQuery::new("User").with_filter(
            FilterExpr::and(vec![
                SimpleFilter::Ge {
                    field: "age".to_string(),
                    value: Value::Int32(30),
                },
                SimpleFilter::Le {
                    field: "age".to_string(),
                    value: Value::Int32(50),
                },
            ])
            .into(),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_eval_like(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/like");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("prefix", |b| {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::like("name", "Alice%").into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("suffix", |b| {
        let query = GraphQuery::new("User").with_filter(FilterExpr::like("email", "%@example0.com").into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("contains", |b| {
        let query = GraphQuery::new("User").with_filter(FilterExpr::like("name", "%_1%").into());

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_eval_in(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/in");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("values", size), &size, |b, &size| {
            let values: Vec<Value> = (0..size).map(|i| Value::Int32(20 + i)).collect();
            let query =
                GraphQuery::new("User").with_filter(FilterExpr::in_values("age", values).into());

            b.iter(|| {
                black_box(executor.execute(&query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_eval_and(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/and");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("2_conditions", |b| {
        let query = GraphQuery::new("User").with_filter(
            FilterExpr::and(vec![
                SimpleFilter::Gt {
                    field: "age".to_string(),
                    value: Value::Int32(25),
                },
                SimpleFilter::Eq {
                    field: "status".to_string(),
                    value: Value::String("active".to_string()),
                },
            ])
            .into(),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("5_conditions", |b| {
        let query = GraphQuery::new("User").with_filter(
            FilterExpr::and(vec![
                SimpleFilter::Gt {
                    field: "age".to_string(),
                    value: Value::Int32(20),
                },
                SimpleFilter::Lt {
                    field: "age".to_string(),
                    value: Value::Int32(70),
                },
                SimpleFilter::Like {
                    field: "email".to_string(),
                    pattern: "%example%".to_string(),
                },
                SimpleFilter::Ne {
                    field: "status".to_string(),
                    value: Value::String("deleted".to_string()),
                },
                SimpleFilter::IsNotNull {
                    field: "name".to_string(),
                },
            ])
            .into(),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_eval_or(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/or");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("2_conditions", |b| {
        let query = GraphQuery::new("User").with_filter(
            FilterExpr::or(vec![
                SimpleFilter::Eq {
                    field: "status".to_string(),
                    value: Value::String("admin".to_string()),
                },
                SimpleFilter::Gt {
                    field: "age".to_string(),
                    value: Value::Int32(60),
                },
            ])
            .into(),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.bench_function("short_circuit", |b| {
        // First condition matches most, should short-circuit
        let query = GraphQuery::new("User").with_filter(
            FilterExpr::or(vec![
                SimpleFilter::Gt {
                    field: "age".to_string(),
                    value: Value::Int32(18),
                },
                SimpleFilter::Eq {
                    field: "status".to_string(),
                    value: Value::String("admin".to_string()),
                },
            ])
            .into(),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

fn bench_eval_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/nested");

    let ctx = TestContext::with_scale(Scale::Medium);
    let executor = ctx.executor();

    group.bench_function("complex", |b| {
        // Complex filter: (age > 30 AND status = 'active' AND email LIKE '%example%')
        // Note: Deeply nested AND/OR structures are not supported in SimpleFilter
        let query = GraphQuery::new("User").with_filter(
            FilterExpr::and(vec![
                SimpleFilter::Gt {
                    field: "age".to_string(),
                    value: Value::Int32(30),
                },
                SimpleFilter::Eq {
                    field: "status".to_string(),
                    value: Value::String("active".to_string()),
                },
                SimpleFilter::Like {
                    field: "email".to_string(),
                    pattern: "%example%".to_string(),
                },
            ])
            .into(),
        );

        b.iter(|| {
            black_box(executor.execute(&query).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_eval_eq,
    bench_eval_range,
    bench_eval_like,
    bench_eval_in,
    bench_eval_and,
    bench_eval_or,
    bench_eval_nested,
);

criterion_main!(benches);
