# Core Concepts

Understanding ORMDB's core concepts will help you get the most out of the database. ORMDB is designed differently from traditional databases - it puts **ORM workloads first**.

---

## What Makes ORMDB Different

Traditional databases expose SQL, and ORMs translate your object-oriented code into SQL queries. This creates an impedance mismatch that leads to common problems like N+1 queries, type mismatches, and error-prone migrations.

ORMDB takes a different approach: **the database understands your object model natively**.

```
Traditional Stack          ORMDB Stack
─────────────────          ──────────────
┌─────────────────┐        ┌─────────────────┐
│   Application   │        │   Application   │
├─────────────────┤        ├─────────────────┤
│      ORM        │        │   ORM Adapter   │
├─────────────────┤        ├─────────────────┤
│ SQL Generation  │        │  Typed Protocol │
├─────────────────┤        ├─────────────────┤
│    SQL DB       │        │     ORMDB       │
└─────────────────┘        └─────────────────┘
```

---

## Key Concepts

### [Graph Queries](graph-queries.md)

Load entities and their relations in a single request. Graph queries eliminate N+1 problems by design, with built-in budget enforcement to prevent runaway queries.

```rust
GraphQuery::new("User")
    .include("posts")        // One-to-many
    .include("posts.comments") // Nested
```

### [Typed Protocol](typed-protocol.md)

Queries are structured data, not SQL strings. This enables compile-time validation, prevents injection attacks, and allows the database to optimize queries more effectively.

```rust
// Type-safe filter expression
FilterExpr::and(
    FilterExpr::eq("status", Value::String("active")),
    FilterExpr::gt("age", Value::Int32(18))
)
```

### [MVCC Versioning](mvcc.md)

Every entity write creates a new version, enabling point-in-time queries, change history, and non-blocking reads. Deleted entities are soft-deleted as tombstones.

```rust
// Get entity at a specific point in time
storage.get_at(&entity_id, timestamp)?
```

### [Storage Architecture](storage-architecture.md)

A hybrid storage system combining row-oriented storage for OLTP workloads with columnar storage for analytics. Multiple index types (hash, B-tree) accelerate different query patterns.

```
Storage Engine
├── Row Store (sled) - MVCC versioned records
├── Columnar Store   - Analytics queries
├── Hash Index       - O(1) equality lookups
└── B-tree Index     - O(log N) range queries
```

### [Migration Safety](migration-safety.md)

Schema changes are analyzed and assigned safety grades (A/B/C/D). This prevents accidental data loss and enables safe online migrations.

| Grade | Impact | Example |
|-------|--------|---------|
| **A** | Non-breaking, online | Add optional field |
| **B** | Online with backfill | Add index |
| **C** | Brief write lock | Narrow type |
| **D** | Destructive | Remove field |

---

## Design Philosophy

### 1. ORM-First

The database is designed around ORM access patterns, not ad-hoc SQL queries. This means:

- **Entity-centric**: Data is organized around entities and their relations
- **Graph fetching**: Load related data in one request
- **Type awareness**: The database understands your schema types

### 2. Safety by Default

Dangerous operations require explicit acknowledgment:

- **Query budgets**: Prevent unbounded result sets
- **Migration grading**: Understand impact before deploying
- **Soft deletes**: Recover from accidental deletions

### 3. Observability

Built-in monitoring and debugging:

- **Query explain**: Understand execution plans
- **Metrics export**: Prometheus-compatible metrics
- **Audit logging**: Track all changes

---

## Next Steps

- **[Graph Queries](graph-queries.md)** - Deep dive into N+1 elimination
- **[Typed Protocol](typed-protocol.md)** - Understanding type-safe queries
- **[MVCC](mvcc.md)** - How versioning works
- **[Storage Architecture](storage-architecture.md)** - Under the hood
- **[Migration Safety](migration-safety.md)** - Safe schema evolution
