# Benchmarks

ORMDB must be evaluated with reproducible benchmarks that reflect ORM workloads. This document defines a baseline methodology and acceptance criteria.

## Principles

- Focus on ORM access patterns, not hand-written SQL.
- Measure end-to-end request latency, not just database time.
- Report p50, p95, and p99.
- Include concurrency and contention scenarios.

## Environment

A benchmark run should specify:

- Hardware (CPU, RAM, storage type).
- Dataset size and distribution.
- Cache state (cold vs warm).
- Concurrency level and connection pool size.

## Workloads

1. Aggregate fetch
   - Root entity with 2-3 relation includes.
   - Measures N+1 elimination and result assembly cost.

2. Transactional write
   - Insert parent + children + update aggregate field.
   - Measures constraint enforcement and transactional latency.

3. Schema migration
   - Add field, backfill, add constraint, add index.
   - Measures online safety and write availability.

4. Cache invalidation
   - Read graph, update a node, verify version token changes.
   - Measures change stream throughput and correctness.

## Metrics

- Latency p50/p95/p99.
- Queries per request (internal + external).
- Bytes over the wire.
- Plan cache hit rate.
- CPU usage per request.
- Migration blocking time.
- Stale read rate (for cache tests).

## Acceptance criteria (initial)

- Bounded internal query count for graph fetches.
- No schema-change induced write outages for grade A/B migrations.
- Stable pagination results across includes.
- Correct cache invalidation using version tokens.

These criteria should be tightened as implementation progresses.
