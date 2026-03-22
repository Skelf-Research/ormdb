# Performance Guide

Optimize ORMDB query performance and resource usage.

## Query Optimization

### Use Field Selection

Only fetch fields you need:

```rust
// Bad: Fetches all fields
let query = GraphQuery::new("User");

// Good: Fetches only needed fields
let query = GraphQuery::new("User")
    .with_fields(vec!["id", "name", "email"]);
```

**Impact:** Reduces data transfer and memory usage by 50-90% for entities with many fields.

### Optimize Filters

Use indexed fields in filters:

```rust
// Bad: Full table scan
let query = GraphQuery::new("User")
    .with_filter(FilterExpr::like("bio", "%developer%"));

// Good: Uses index
let query = GraphQuery::new("User")
    .with_filter(FilterExpr::eq("status", Value::String("active".into())));
```

**Filter Performance:**

| Operator | Index Usage | Performance |
|----------|-------------|-------------|
| `eq` | Yes | Fast |
| `in` | Yes | Fast |
| `lt`, `gt`, `le`, `ge` | Yes (range) | Fast |
| `like "prefix%"` | Yes (prefix) | Fast |
| `like "%suffix"` | No | Slow |
| `ilike` | Depends | Varies |

### Limit Include Depth

Deep includes cause exponential data loading:

```rust
// Bad: Could load millions of rows
let query = GraphQuery::new("User")
    .include(RelationInclude::new("posts")
        .include(RelationInclude::new("comments")
            .include(RelationInclude::new("author")
                .include(RelationInclude::new("posts")))));

// Good: Limited depth with filters
let query = GraphQuery::new("User")
    .include(RelationInclude::new("posts")
        .with_limit(10)
        .with_order(OrderSpec::desc("created_at"))
        .include(RelationInclude::new("comments")
            .with_limit(5)));
```

### Use Query Budgets

Prevent runaway queries:

```rust
let query = GraphQuery::new("User")
    .with_budget(FanoutBudget {
        max_entities: 1000,
        max_edges: 5000,
        max_depth: 3,
    });
```

## Indexing Strategies

### Creating Indexes

```rust
// Single field index
let idx = IndexDef::new("user_email_idx", vec!["email"])
    .unique();

// Composite index
let idx = IndexDef::new("post_author_date_idx", vec!["author_id", "created_at"]);

// Partial index
let idx = IndexDef::new("active_users_idx", vec!["email"])
    .with_filter(FilterExpr::eq("status", Value::String("active".into())));

schema.add_index("User", idx);
```

### Index Selection Guidelines

| Query Pattern | Recommended Index |
|---------------|-------------------|
| Equality on single field | Single field index |
| Equality + range | Composite (equality fields first) |
| Multiple equality fields | Composite index |
| Foreign key lookups | Index on FK field |
| Sorting | Index on sort field |

### Analyzing Index Usage

```bash
# Show index statistics
ormdb admin stats --indexes

# Explain query plan
ormdb query User "status = 'active'" --explain
```

**Example Output:**
```
Query Plan:
  └─ Index Scan: user_status_idx
     Filter: status = 'active'
     Estimated rows: 5,000
     Index selectivity: 0.05
```

## Caching

### Server-Side Caching

```toml
# ormdb.toml
[storage]
cache_size_mb = 512

[query]
result_cache_size_mb = 128
result_cache_ttl_seconds = 60
```

### Query Result Caching

```rust
// Enable caching for specific query
let query = GraphQuery::new("User")
    .with_cache(CachePolicy::new()
        .ttl(Duration::from_secs(60))
        .key("active_users"));

// Invalidate cache
client.invalidate_cache("active_users").await?;
```

### Client-Side Caching

=== "TypeScript"

    ```typescript
    const client = new OrmdbClient({
      baseUrl: "http://localhost:8080",
      cache: {
        enabled: true,
        maxSize: 1000,
        ttl: 60000, // 60 seconds
      },
    });
    ```

=== "Python"

    ```python
    client = OrmdbClient(
        base_url="http://localhost:8080",
        cache_enabled=True,
        cache_max_size=1000,
        cache_ttl=60,
    )
    ```

## Batch Operations

### Batch Inserts

```rust
// Bad: Individual inserts
for user in users {
    client.insert("User", user).await?;
}

// Good: Batch insert
let mutation = InsertMutation::new("User")
    .with_batch(users);
client.mutate(Mutation::Insert(mutation)).await?;
```

**Impact:** 10-100x faster for large datasets.

### Batch Queries

```rust
// Bad: N+1 queries
for user_id in user_ids {
    let posts = client.query(GraphQuery::new("Post")
        .with_filter(FilterExpr::eq("author_id", user_id))).await?;
}

// Good: Single query with IN
let query = GraphQuery::new("Post")
    .with_filter(FilterExpr::in_values("author_id", user_ids.clone()));
let posts = client.query(query).await?;
```

## Connection Pooling

### Rust

```rust
let config = ClientConfig {
    max_connections: 20,
    min_connections: 5,
    connection_timeout: Duration::from_secs(5),
    idle_timeout: Duration::from_secs(300),
    ..Default::default()
};

let client = Client::with_config(config)?;
```

### TypeScript

```typescript
const client = new OrmdbClient({
  baseUrl: "http://localhost:8080",
  pool: {
    maxConnections: 20,
    minConnections: 5,
    idleTimeout: 300000,
  },
});
```

## Monitoring Performance

### Query Metrics

```bash
# View query statistics
ormdb admin stats --queries

# Output:
# Query Statistics (last hour):
#   Total queries: 125,432
#   Avg latency: 2.3ms
#   P50 latency: 1.2ms
#   P95 latency: 8.5ms
#   P99 latency: 45.2ms
#
# Slow queries (>100ms):
#   - User with posts (avg 250ms, count: 42)
#   - Post full-text search (avg 180ms, count: 156)
```

### Prometheus Metrics

```toml
# ormdb.toml
[metrics]
enabled = true
port = 9090
```

**Available metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `ormdb_query_duration_seconds` | Histogram | Query latency |
| `ormdb_query_total` | Counter | Total queries |
| `ormdb_query_rows_returned` | Histogram | Rows per query |
| `ormdb_cache_hits_total` | Counter | Cache hits |
| `ormdb_cache_misses_total` | Counter | Cache misses |
| `ormdb_connections_active` | Gauge | Active connections |

### Slow Query Log

```toml
# ormdb.toml
[logging]
slow_query_threshold_ms = 100
slow_query_log = "/var/log/ormdb/slow.log"
```

## Performance Tuning Checklist

### Query Level

- [ ] Select only needed fields
- [ ] Use indexed fields in filters
- [ ] Limit include depth (max 3-4)
- [ ] Use pagination for large results
- [ ] Set appropriate query budgets
- [ ] Use batch operations for bulk data

### Index Level

- [ ] Create indexes for frequently filtered fields
- [ ] Create composite indexes for multi-field filters
- [ ] Index foreign key fields
- [ ] Remove unused indexes
- [ ] Monitor index usage statistics

### Server Level

- [ ] Configure appropriate cache size
- [ ] Enable result caching for hot queries
- [ ] Set up connection pooling
- [ ] Monitor query metrics
- [ ] Review slow query log regularly

### Client Level

- [ ] Use connection pooling
- [ ] Implement client-side caching
- [ ] Handle pagination properly
- [ ] Avoid N+1 query patterns

## Benchmarking

### Built-in Benchmarks

```bash
# Run performance benchmarks
ormdb benchmark --entities 100000 --queries 10000

# Output:
# Benchmark Results:
#   Insert (single): 15,234 ops/sec
#   Insert (batch 100): 892,341 rows/sec
#   Query (simple): 45,234 ops/sec
#   Query (with include): 12,341 ops/sec
#   Query (aggregate): 28,456 ops/sec
```

### Custom Benchmarks

```rust
use std::time::Instant;

let start = Instant::now();
for _ in 0..1000 {
    client.query(query.clone()).await?;
}
let duration = start.elapsed();
println!("Avg query time: {:?}", duration / 1000);
```

## Common Performance Issues

### Issue: Slow Queries with Many Includes

**Symptom:** Queries with deep includes take seconds.

**Solution:**
```rust
// Add limits to includes
.include(RelationInclude::new("posts")
    .with_limit(10)
    .with_filter(FilterExpr::eq("published", true)))
```

### Issue: High Memory Usage

**Symptom:** Server memory grows unbounded.

**Solution:**
```toml
[query]
max_entities = 5000  # Reduce from default 10000
result_cache_size_mb = 64  # Limit cache size
```

### Issue: Connection Exhaustion

**Symptom:** "Too many connections" errors.

**Solution:**
```rust
// Use connection pooling with limits
let config = ClientConfig {
    max_connections: 20,
    connection_timeout: Duration::from_secs(5),
    ..Default::default()
};
```

### Issue: N+1 Queries

**Symptom:** Many small queries instead of one large query.

**Solution:**
```rust
// Use includes instead of separate queries
let query = GraphQuery::new("User")
    .include(RelationInclude::new("posts")
        .include(RelationInclude::new("comments")));
```

---

## Next Steps

- **[Graph Queries](../concepts/graph-queries.md)** - Understand query optimization
- **[Index Internals](../architecture/index-internals.md)** - How indexes work
- **[Monitoring](../operations/monitoring.md)** - Track performance in production
