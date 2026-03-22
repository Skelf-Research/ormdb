# Architecture Overview

This section provides a deep dive into ORMDB's internal architecture for advanced users and contributors.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Client Layer                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │ Rust Client │  │  TS Client  │  │  Py Client  │  ORM Adapters   │
│  └─────────────┘  └─────────────┘  └─────────────┘                  │
├─────────────────────────────────────────────────────────────────────┤
│                           Protocol Layer                             │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  ormdb-proto: GraphQuery, Mutation, Value, Result types     │    │
│  │  Wire format: rkyv serialization                            │    │
│  └─────────────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────────────┤
│                           Server Layer                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │   Gateway    │  │   Server     │  │     CLI      │              │
│  │  (HTTP API)  │  │  (TCP/IPC)   │  │  (Commands)  │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
├─────────────────────────────────────────────────────────────────────┤
│                           Core Layer                                 │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                      ormdb-core                               │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────┐  │   │
│  │  │  Catalog   │ │   Query    │ │  Storage   │ │ Security  │  │   │
│  │  │            │ │  Engine    │ │  Engine    │ │           │  │   │
│  │  └────────────┘ └────────────┘ └────────────┘ └───────────┘  │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────┐  │   │
│  │  │ Migration  │ │ Constraint │ │Replication │ │  Metrics  │  │   │
│  │  │  Engine    │ │ Validation │ │    (CDC)   │ │           │  │   │
│  │  └────────────┘ └────────────┘ └────────────┘ └───────────┘  │   │
│  └──────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│                         Storage Backend                              │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                        sled                                  │    │
│  │           (Embedded key-value store with ACID)               │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Crate Structure

ORMDB is organized as a Cargo workspace with multiple crates:

| Crate | Purpose |
|-------|---------|
| `ormdb-core` | Storage engine, query executor, catalog, security, migrations |
| `ormdb-proto` | Protocol definitions (GraphQuery, Mutation, Value, etc.) |
| `ormdb-client` | Rust client library |
| `ormdb-server` | Standalone database server |
| `ormdb-gateway` | HTTP API gateway |
| `ormdb-cli` | Command-line interface |
| `ormdb-lang` | Schema language parser |
| `ormdb-bench` | Benchmarking suite |

### Dependency Graph

```
ormdb-cli ─────────────┐
                       │
ormdb-gateway ─────────┼──► ormdb-server ──► ormdb-core
                       │                           │
ormdb-client ──────────┘                           ▼
       │                                      ormdb-proto
       └───────────────────────────────────────────┘
```

---

## Core Components

### Catalog

The catalog stores schema metadata:

```rust
pub struct Catalog {
    db: sled::Db,
    entities: Tree,      // Entity definitions
    relations: Tree,     // Relation definitions
    constraints: Tree,   // Constraint definitions
    versions: Tree,      // Schema version history
}
```

Key responsibilities:
- Store entity, relation, and constraint definitions
- Version schema changes
- Validate queries against schema
- Provide schema introspection

### Query Engine

The query engine transforms and executes queries:

```
GraphQuery ──► Planner ──► QueryPlan ──► Executor ──► Result
                  │                          │
                  ▼                          ▼
              Catalog                    Storage
```

Components:
- **Planner** (`query/planner.rs`): Resolves schema, validates queries, produces plans
- **Executor** (`query/executor.rs`): Runs plans against storage
- **Filter** (`query/filter.rs`): Evaluates filter expressions
- **Join** (`query/join.rs`): Executes relation includes
- **Aggregate** (`query/aggregate.rs`): COUNT, SUM, AVG, etc.
- **Cache** (`query/cache.rs`): Plan caching by fingerprint

### Storage Engine

The storage engine manages data persistence:

```
Storage Engine
├── Data Tree (versioned records)
├── Meta Tree (latest pointers)
├── Type Index Tree (entity type index)
├── Columnar Store (analytics)
├── Hash Index (equality lookups)
└── B-tree Index (range lookups)
```

Key features:
- MVCC versioning
- Multiple index types
- Hybrid row/columnar storage
- Soft deletes

### Security

Security subsystem components:

- **Capabilities** (`security/capabilities.rs`): Per-connection permissions
- **RLS** (`security/rls.rs`): Row-level security policies
- **Field Masking** (`security/field_security.rs`): Sensitive data masking
- **Audit** (`security/audit.rs`): Change logging
- **Budget** (`security/budget.rs`): Query resource limits

### Migration Engine

Schema evolution components:

- **Diff** (`migration/diff.rs`): Compute schema differences
- **Grader** (`migration/grader.rs`): Assign safety grades
- **Plan** (`migration/plan.rs`): Generate migration steps
- **Executor** (`migration/executor.rs`): Apply migrations
- **Backfill** (`migration/backfill.rs`): Background data updates

---

## Request Flow

### Query Request

```
1. Client sends GraphQuery
         │
         ▼
2. Server validates authentication
         │
         ▼
3. Query Planner validates against Catalog
         │
         ▼
4. Security applies RLS policies
         │
         ▼
5. Executor runs plan
   ├── Fetch root entities from Storage
   ├── Apply filters
   ├── Execute includes (joins)
   └── Project fields
         │
         ▼
6. Apply field masking
         │
         ▼
7. Return Result to client
```

### Mutation Request

```
1. Client sends Mutation
         │
         ▼
2. Server validates authentication
         │
         ▼
3. Validate mutation against Catalog
         │
         ▼
4. Check capabilities (write permission)
         │
         ▼
5. Validate constraints
   ├── Unique constraints
   ├── Foreign key constraints
   └── Check constraints
         │
         ▼
6. Execute mutation
   ├── Write to Storage (new version)
   ├── Update indexes
   ├── Update columnar store
   └── Append to changelog
         │
         ▼
7. Record in audit log
         │
         ▼
8. Return Result to client
```

---

## Concurrency Model

ORMDB uses a multi-threaded async model:

```
┌─────────────────────────────────────────────────────────────┐
│                     Tokio Runtime                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  Acceptor   │  │   Worker    │  │   Worker    │  ...     │
│  │   Task      │  │   Task      │  │   Task      │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│         │                │                │                  │
│         ▼                ▼                ▼                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Shared State (Arc<RwLock>)              │    │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────────────────┐  │    │
│  │  │ Catalog │  │ Storage │  │ Connection Pool     │  │    │
│  │  └─────────┘  └─────────┘  └─────────────────────┘  │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Read Operations

- Non-blocking due to MVCC
- Multiple readers can proceed concurrently
- Uses `RwLock` read guards

### Write Operations

- Acquire write lock on affected trees
- Create new version (append-only)
- Update metadata atomically
- Release lock

---

## Wire Protocol

### Message Format

```
┌──────────────────────────────────────────────────────────────┐
│                        Header (8 bytes)                       │
│  ┌──────────────┬──────────────┬──────────────────────────┐  │
│  │ Magic (4B)   │ Version (2B) │ Message Type (2B)        │  │
│  │ 0x4F524D44   │ 0x0001       │ Query/Mutation/Result    │  │
│  └──────────────┴──────────────┴──────────────────────────┘  │
├──────────────────────────────────────────────────────────────┤
│                        Body (variable)                        │
│  ┌──────────────────────────────────────────────────────┐    │
│  │              rkyv-serialized payload                  │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### Message Types

| Type | Code | Description |
|------|------|-------------|
| Handshake | 0x01 | Protocol negotiation |
| Query | 0x02 | GraphQuery request |
| Mutation | 0x03 | Mutation request |
| Result | 0x04 | Query/mutation result |
| Error | 0x05 | Error response |
| Ping | 0x06 | Keepalive |
| Pong | 0x07 | Keepalive response |

---

## Metrics and Observability

### Prometheus Metrics

ORMDB exports metrics in Prometheus format:

```
# Query metrics
ormdb_queries_total{entity="User",operation="find"}
ormdb_query_duration_seconds{entity="User",quantile="0.99"}

# Storage metrics
ormdb_storage_size_bytes
ormdb_entities_total{type="User"}

# Connection metrics
ormdb_connections_active
ormdb_connections_total
```

### Tracing

OpenTelemetry-compatible tracing:

```rust
#[tracing::instrument]
async fn execute_query(query: GraphQuery) -> Result<QueryResult> {
    // Spans are automatically created
}
```

---

## Detailed Architecture Guides

- **[Query Engine](query-engine.md)** - Planning, execution, caching
- **[Storage Engine](storage-engine.md)** - Data organization, indexes
- **[Index Internals](index-internals.md)** - Hash and B-tree implementation

---

## Contributing

See the [Contributing Guide](https://github.com/Skelf-Research/ormdb/blob/main/CONTRIBUTING.md) for:

- Development setup
- Code style guidelines
- Testing requirements
- Pull request process
