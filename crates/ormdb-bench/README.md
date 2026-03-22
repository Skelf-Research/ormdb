# ormdb-bench

Benchmarking suite for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-bench` contains comprehensive benchmarks for ORMDB performance testing, including:

- **Component Benchmarks** - Storage, query, mutation, serialization
- **Feature Benchmarks** - Filtering, joins, caching
- **Comparison Benchmarks** - vs SQLite, vs PostgreSQL
- **End-to-End Benchmarks** - Full request/response cycles

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p ormdb-bench

# Run specific benchmark
cargo bench -p ormdb-bench --bench storage
cargo bench -p ormdb-bench --bench query
cargo bench -p ormdb-bench --bench vs_sqlite

# Run PostgreSQL comparison (requires running PostgreSQL)
cargo bench -p ormdb-bench --bench vs_postgres --features postgres

# Generate HTML reports
cargo bench -p ormdb-bench -- --save-baseline main
```

## Benchmark Categories

### Storage (`storage`)
- Insert throughput
- Read latency
- Index performance
- Transaction overhead

### Query (`query`)
- Simple fetches
- Graph fetches with includes
- Filter evaluation
- Pagination

### Mutation (`mutation`)
- Create operations
- Update operations
- Delete with cascades
- Batch mutations

### Serialization (`serialization`)
- Protocol encoding
- Zero-copy deserialization
- Large payload handling

### Comparisons

| Benchmark | Description |
|-----------|-------------|
| `vs_sqlite` | Compare against SQLite for embedded workloads |
| `vs_postgres` | Compare against PostgreSQL for graph fetches |

## Results

Benchmark results are saved to `target/criterion/`. View HTML reports at:
```
target/criterion/report/index.html
```

## Methodology

See [`docs/benchmarks.md`](../../docs/benchmarks.md) for detailed methodology and acceptance criteria.

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
