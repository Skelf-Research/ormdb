# ORMDB vs PostgreSQL

This guide compares ORMDB with PostgreSQL to help you understand when each is the right choice.

---

## Overview

| Aspect | ORMDB | PostgreSQL |
|--------|-------|------------|
| **Type** | ORM-first embedded DB | Full-featured SQL server |
| **Deployment** | Embedded or standalone | Server process |
| **Query Language** | Typed protocol | SQL |
| **Primary Strength** | ORM workloads | General purpose |
| **Ecosystem** | Growing | Massive |

---

## Different Design Goals

### PostgreSQL

PostgreSQL is designed to be a **general-purpose relational database**:

- Full SQL compliance
- Extensible with custom types, functions, operators
- Advanced features (JSON, full-text search, GIS)
- Decades of production hardening
- Massive ecosystem of tools and extensions

### ORMDB

ORMDB is designed for **ORM-centric application backends**:

- ORM patterns are first-class
- Graph queries eliminate N+1
- Type-safe protocol instead of SQL
- Built-in security and audit
- Simpler operational model

---

## When to Choose PostgreSQL

**Choose PostgreSQL when:**

- You need full SQL power (CTEs, window functions, recursive queries)
- Advanced data types are required (PostGIS, arrays, JSON operators)
- Your team has strong SQL expertise
- You need extensive extension ecosystem (pg_stat, pg_repack, etc.)
- Running a data warehouse or analytics platform
- Maximum compatibility with existing tools

**Example use cases:**
- Complex reporting with window functions
- Geospatial applications (PostGIS)
- Full-text search at scale
- Data warehousing (with Citus or TimescaleDB)

---

## When to Choose ORMDB

**Choose ORMDB when:**

- Your application is ORM-driven
- N+1 queries are a persistent problem
- You want compile-time query validation
- Schema migration safety is important
- Row-level security needs to be enforced
- Simpler deployment is preferred (embedded option)

**Example use cases:**
- SaaS application backends
- Multi-tenant applications
- Mobile app backends
- Microservices with entity-centric data

---

## Feature Comparison

### Query Capabilities

| Feature | PostgreSQL | ORMDB |
|---------|------------|-------|
| Basic CRUD | Yes | Yes |
| Filters | SQL WHERE | FilterExpr |
| JOINs | Full SQL JOINs | Graph includes |
| Aggregations | Full SQL | COUNT, SUM, AVG, MIN, MAX |
| Window functions | Yes | Not yet |
| CTEs | Yes | Not yet |
| Subqueries | Yes | Via includes |
| Recursive queries | Yes | Not yet |
| Full-text search | Yes (tsvector) | Not yet |
| JSON queries | Yes (JSONB) | Via JSON fields |
| Custom functions | Yes (PL/pgSQL) | No |

### Data Types

| Type | PostgreSQL | ORMDB |
|------|------------|-------|
| Integers | int2, int4, int8 | Int32, Int64 |
| Floats | float4, float8, numeric | Float32, Float64 |
| Strings | text, varchar | String |
| Boolean | boolean | Bool |
| UUID | uuid | Uuid |
| Timestamp | timestamp, timestamptz | Timestamp |
| Binary | bytea | Bytes |
| JSON | json, jsonb | Json |
| Arrays | Any type[] | Array(Scalar) |
| Enums | CREATE TYPE | Enum |
| Custom types | Yes | No |
| Geometric | point, polygon, etc. | No |
| Network | inet, cidr, macaddr | No |
| Range types | int4range, tsrange, etc. | No |

### Security

| Feature | PostgreSQL | ORMDB |
|---------|------------|-------|
| Authentication | Multiple methods | Token-based |
| Row-level security | Policies | Built-in policies |
| Column-level security | Grants | Field masking |
| Audit logging | pgaudit extension | Built-in |
| Query limits | statement_timeout | Query budgets |
| SSL/TLS | Yes | Yes |

### Operations

| Feature | PostgreSQL | ORMDB |
|---------|------------|-------|
| Backup | pg_dump, pg_basebackup | Built-in |
| Replication | Streaming, logical | CDC-based |
| Monitoring | pg_stat_* views | Prometheus export |
| Connection pooling | pgbouncer | Built-in |
| High availability | Patroni, pgpool | Planned |
| Sharding | Citus | Planned |

---

## Performance Characteristics

### Simple Queries

| Operation | PostgreSQL | ORMDB |
|-----------|------------|-------|
| Point lookup (PK) | ~0.1ms | ~0.05ms |
| Index scan (1K) | ~1ms | ~1ms |
| Full scan (10K) | ~10ms | ~15ms |

PostgreSQL has slight edge for simple queries due to mature optimization.

### ORM-Style Queries

| Operation | PostgreSQL (ORM) | ORMDB |
|-----------|------------------|-------|
| User + posts (eager) | ~2ms (JOIN) | ~2ms |
| User + posts (lazy) | ~50ms (N+1) | ~2ms |
| 3-level include | ~10ms (manual) | ~5ms |

ORMDB excels when query patterns match ORM access patterns.

### Write Performance

| Operation | PostgreSQL | ORMDB |
|-----------|------------|-------|
| Single insert | ~0.2ms | ~0.1ms |
| Batch 1000 | ~5ms | ~15ms |
| Update | ~0.2ms | ~0.1ms |

PostgreSQL's mature WAL is highly optimized for writes.

---

## Deployment Comparison

### PostgreSQL

```yaml
# docker-compose.yml
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: secret
    volumes:
      - pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"
```

Requirements:
- Separate server process
- Connection pooling (pgbouncer)
- Backup strategy (pg_dump, WAL archiving)
- Monitoring (pg_stat, pgBadger)
- High availability setup (Patroni, etc.)

### ORMDB Embedded

```rust
// Embedded in application
let storage = StorageEngine::open(StorageConfig::new("./data"))?;
let catalog = Catalog::open(storage.db())?;

// No separate process
// No connection management
// Built-in backup
```

### ORMDB Server

```yaml
# docker-compose.yml
services:
  ormdb:
    image: ormdb/ormdb:latest
    volumes:
      - ormdb_data:/data
    ports:
      - "8080:8080"
```

Simpler operational model:
- Built-in connection pooling
- Built-in metrics export
- Built-in backup API

---

## SQL vs Typed Protocol

### PostgreSQL (SQL)

```python
# Python with psycopg2
cursor.execute("""
    SELECT u.id, u.name, p.id, p.title
    FROM users u
    LEFT JOIN posts p ON p.author_id = u.id
    WHERE u.status = %s
    ORDER BY u.name
    LIMIT 20
""", ['active'])

# Downsides:
# - SQL is a string (no compile-time checking)
# - Must manually map results to objects
# - Type coercion happens at runtime
```

### ORMDB (Typed Protocol)

```rust
let query = GraphQuery::new("User")
    .with_filter(FilterExpr::eq("status", Value::String("active".into())))
    .with_order(OrderSpec::asc("name"))
    .with_pagination(Pagination::new(20, 0))
    .include(RelationInclude::new("posts")
        .with_fields(vec!["id", "title"]));

// Benefits:
// - Schema validated before execution
// - Type-safe filter values
// - Structured results
```

---

## Migration Comparison

### PostgreSQL

```sql
-- Manual migration file
BEGIN;

ALTER TABLE users ADD COLUMN status VARCHAR(20);
UPDATE users SET status = 'active' WHERE status IS NULL;
ALTER TABLE users ALTER COLUMN status SET NOT NULL;

COMMIT;
```

Or with tools like Flyway, Liquibase, or Alembic:

```python
# Alembic migration
def upgrade():
    op.add_column('users', sa.Column('status', sa.String(20)))
    op.execute("UPDATE users SET status = 'active'")
    op.alter_column('users', 'status', nullable=False)
```

### ORMDB

```rust
let schema_v2 = current_schema.with_change(|users| {
    users.add_field(
        FieldDef::new("status", ScalarType::String)
            .with_default(DefaultValue::String("active".into()))
    )
});

// Safety grade computed automatically
let grade = SafetyGrader::grade(&diff);
// Grade: B - Safe with backfill

// Backfill handled by database
catalog.apply_schema(schema_v2)?;
```

---

## Extension Ecosystem

### PostgreSQL Extensions

PostgreSQL's extension ecosystem is a major strength:

| Extension | Purpose |
|-----------|---------|
| PostGIS | Geospatial |
| pg_trgm | Fuzzy text search |
| pgvector | Vector similarity |
| TimescaleDB | Time-series |
| Citus | Distributed PostgreSQL |
| pgaudit | Audit logging |
| pg_stat_statements | Query statistics |

### ORMDB Extensibility

ORMDB focuses on core ORM features:

- Built-in audit logging
- Built-in metrics
- Built-in CDC
- Plugin system (planned)

For specialized needs (geospatial, vectors), PostgreSQL is the better choice.

---

## Hybrid Architecture

You can use both databases for different purposes:

```
┌─────────────────────────────────────────────────────────────┐
│                      Application                              │
├─────────────────────────────────┬───────────────────────────┤
│           ORMDB                 │        PostgreSQL          │
│                                 │                            │
│  - User management              │  - Analytics/reporting     │
│  - Orders/transactions          │  - Full-text search        │
│  - Real-time features           │  - Geospatial queries      │
│  - Multi-tenant data            │  - Data warehouse          │
│                                 │                            │
│  Row-level security             │  Complex SQL               │
│  Graph queries                  │  Extensions                │
│  Type-safe protocol             │  SQL power                 │
└─────────────────────────────────┴───────────────────────────┘
```

### Sync Patterns

```rust
// CDC from ORMDB to PostgreSQL for analytics
let changelog = ormdb.subscribe_changes("Order")?;

for change in changelog {
    match change.operation {
        Operation::Insert => pg.insert("orders_analytics", &change.fields),
        Operation::Update => pg.update("orders_analytics", &change.fields),
        Operation::Delete => pg.delete("orders_analytics", change.id),
    }
}
```

---

## Cost Comparison

### PostgreSQL

- **Self-hosted**: Server costs + operational overhead
- **Managed (AWS RDS)**: ~$50-500+/month depending on size
- **Managed (Supabase, Neon)**: Free tier to ~$25+/month

### ORMDB

- **Embedded**: No additional cost (part of application)
- **Self-hosted server**: Server costs only
- **Managed**: Coming soon

---

## Summary Table

| Aspect | Choose PostgreSQL | Choose ORMDB |
|--------|------------------|--------------|
| Query flexibility | Need full SQL | ORM patterns sufficient |
| Data types | Need PostGIS, vectors, etc. | Standard types sufficient |
| Team expertise | Strong SQL skills | ORM-oriented developers |
| Extensions | Need pg extensions | Core features sufficient |
| Deployment | Have DB ops expertise | Prefer simpler operations |
| Security model | RLS via policies OK | Want built-in tenant isolation |
| Type safety | Runtime validation OK | Want compile-time checks |

---

## Migration Path

### PostgreSQL to ORMDB

1. Map PostgreSQL tables to ORMDB entities
2. Export data to JSON format
3. Import into ORMDB
4. Update application queries
5. Set up CDC for gradual migration

### ORMDB to PostgreSQL

1. Generate CREATE TABLE statements from catalog
2. Export entities to SQL INSERTs
3. Lose MVCC history, RLS policies
4. Rewrite queries as SQL

---

## Next Steps

- **[vs SQLite](vs-sqlite.md)** - Compare with embedded databases
- **[vs Traditional ORMs](vs-traditional-orms.md)** - Compare with ORM patterns
- **[Getting Started](../getting-started/quickstart.md)** - Try ORMDB
