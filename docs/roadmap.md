# Roadmap

This document defines the implementation roadmap for ORMDB. Each phase includes deliverables, success criteria, and mapping to core technologies.

## Core Technologies

| Crate | Role |
|-------|------|
| **sled** | Embedded storage engine |
| **rkyv** | Zero-copy serialization |
| **mimalloc** | Global memory allocator |
| **async-nng** | Async messaging transport |
| **pest/nom** | Query language parsing |
| **rustyline** | Interactive REPL |
| **clap** | CLI argument parsing |

See `technology.md` for detailed rationale.

---

## Phase 0: Project Bootstrap

**Core tech:** mimalloc, tokio

### Deliverables
- Cargo.toml with all core dependencies declared
- Crate structure with `mimalloc` as global allocator
- CI pipeline (cargo check, clippy, test)

### Success Criteria
- Project compiles with all dependencies
- mimalloc active as global allocator
- CI passes on all targets

---

## Phase 1: Storage Foundation

**Core tech:** sled

### Deliverables
- sled-based key-value storage abstraction
- MVCC version key encoding `(entity_id, version_ts)`
- Basic transaction support via sled transactions
- WAL configuration for durability

### Success Criteria
- Can store and retrieve versioned records
- Transactions commit or rollback atomically
- Data persists across process restarts

---

## Phase 2: Semantic Catalog

**Core tech:** sled (metadata storage), rkyv (catalog serialization)

### Deliverables
- Entity catalog: types, fields, identity definitions
- Relation catalog: cardinality, edge semantics
- Constraint catalog: uniqueness, checks, foreign keys
- Schema versioning and history tracking

### Success Criteria
- Can define entities with typed fields
- Can define relations between entities
- Constraints stored and retrievable by version

---

## Phase 3: Protocol & Serialization

**Core tech:** rkyv

### Deliverables
- Query IR types with `Archive`, `Serialize`, `Deserialize` derives
- Entity block and edge block result structures
- Protocol version negotiation types
- Zero-copy deserialization implementation

### Success Criteria
- Round-trip serialization of query IR works correctly
- Zero-copy reads from byte buffers without allocation
- Protocol version included in all wire messages

---

## Phase 4: Query Engine

**Core tech:** sled (execution), rkyv (result assembly)

### Deliverables
- Graph fetch compiler: model queries to relational plans
- Cost model for query planning decisions
- Plan cache keyed by IR signature
- Execution runtime with join strategies (nested loop, hash)
- Fanout budgeting and limit enforcement

### Success Criteria
- Single-entity queries execute correctly
- Multi-relation graph fetches return correct results
- N+1 query patterns eliminated
- Fanout limits enforced, queries rejected when exceeded

---

## Phase 5: Transport Layer

**Core tech:** async-nng

### Deliverables
- Request-reply pattern for graph fetch queries
- Pub-sub pattern for change streams
- TCP transport for remote clients
- IPC transport for local high-performance connections
- Connection pooling and multiplexing

### Success Criteria
- Clients can send queries and receive structured results
- Change streams deliver entity and relation deltas
- Multiple concurrent connections handled correctly

---

## Phase 6: Transactions & Constraints

**Core tech:** sled (transactions)

### Deliverables
- Full ACID transaction semantics
- Constraint enforcement at write time
- Cascading deletes implementation
- Soft delete support
- Optimistic concurrency control

### Success Criteria
- Constraint violations rejected with clear errors
- Transactions provide proper isolation
- Cascades execute atomically within transaction

---

## Phase 7: Migration Engine

**Core tech:** sled (schema storage), rkyv (bundle serialization)

### Deliverables
- Schema bundle diffing algorithm
- Safety grade calculation (A/B/C/D)
- Expand/contract workflow orchestration
- Background backfill job execution
- Compatibility window support for multi-version deployments

### Success Criteria
- Grade A migrations complete online without blocking writes
- Grade B migrations backfill in background without downtime
- Rollback capability for failed migrations

---

## Phase 8: Security & Policies

**Core tech:** cross-cutting (integrated into query engine and catalog)

### Deliverables
- Capability-based access control per connection
- Row-level security (RLS) compilation into query plans
- Field-level masking for sensitive data
- Query shape limits: depth, fanout, total rows/edges
- Structured audit logging

### Success Criteria
- Unauthorized reads blocked at engine level
- RLS filters applied to all query execution paths
- Sensitive fields masked or omitted based on capabilities
- Complete audit trail for compliance

---

## Phase 9: Query Language

**Core tech:** pest/nom (parsing), ormdb-proto (IR target)

### Deliverables
- ORM-style query language parser (Prisma-like DSL)
- Query language to GraphQuery IR compiler
- Mutation language support (insert, update, delete, upsert)
- Schema introspection queries
- Error reporting with source locations

### Query Language Examples
```
User.findMany().where(status == "active")
User.findMany().include(posts).orderBy(createdAt.desc).limit(10)
User.create({ name: "Alice", email: "alice@example.com" })
User.update().where(id == "abc-123").set({ status: "inactive" })
```

### Success Criteria
- Parse valid queries without panics
- Compile to correct GraphQuery IR
- Meaningful error messages with line/column info
- Round-trip: parse -> compile -> execute -> results

---

## Phase 10: CLI Tool

**Core tech:** clap (CLI), rustyline (REPL), ormdb-client, ormdb-lang

### Deliverables
- Interactive REPL with history and tab completion
- Script file execution mode
- Connection management (connect, disconnect, status)
- Output formatting (table, JSON, CSV)
- Query history persistence

### CLI Examples
```bash
ormdb-cli --host tcp://localhost:9000              # Start REPL
ormdb-cli --host tcp://localhost:9000 -f script.ormql  # Run script
ormdb-cli --host tcp://localhost:9000 -c "User.findMany()"  # Single query
```

### REPL Commands
```
.connect, .disconnect, .status, .schema, .format, .history, .help, .exit
```

### Success Criteria
- REPL starts and accepts queries
- Tab completion for entity and field names
- Query history persists across sessions
- Scripts execute sequentially
- Graceful error handling without crashes

---

## Phase 11: Observability

**Core tech:** tracing

### Deliverables
- Query explanation (EXPLAIN command)
- Metrics collection: query count, fanout usage, cache hit rate
- Structured logging with tracing
- Performance profiling hooks

### Success Criteria
- Can explain any query plan in detail
- Metrics exportable for external monitoring
- Audit logs queryable for debugging and compliance

---

## Phase 12: Hybrid Storage & Compaction

**Core tech:** sled, rkyv

### Deliverables
- **Version Retention & Compaction**
  - Configurable retention policy (TTL or version count)
  - Background compaction thread
  - Tombstone cleanup after retention period
  - `StorageEngine::compact()` API
  - Compaction metrics (versions cleaned, space reclaimed)

- **Columnar Projections (Hybrid Storage)**
  - Automatic columnar projection for all entities
  - Dictionary encoding for string columns
  - Projection pushdown in query executor
  - Dual-write: row store + columnar store on every mutation

- **Analytical Query Support**
  - Aggregate functions: COUNT, SUM, AVG, MIN, MAX
  - GROUP BY compilation to query plan
  - HAVING clause evaluation
  - Columnar scan optimization

### Query Language Extensions
```
User.count()
User.count().where(status == "active")
User.aggregate({ count: true, sum: "balance", avg: "age" })
User.groupBy("department").aggregate({ count: true, avg: "salary" })
```

### Success Criteria
- Old versions cleaned up based on retention policy
- Tombstones removed after retention period
- `size_on_disk()` decreases after compaction
- Compaction metrics exposed
- `User.count()` works with columnar optimization
- GROUP BY queries execute correctly
- Projection pushdown reduces I/O

---

## Phase 13: CDC & Replication

**Core tech:** sled, async-nng, rkyv

### Deliverables
- **Change Data Capture (CDC)**
  - Persistent changelog tree with LSN ordering
  - Change events emitted on every mutation
  - Field-level diff tracking for UPDATEs
  - Full before/after records (enables point-in-time recovery)
  - Subscription filtering by entity type

- **WAL Streaming**
  - Log Sequence Number (LSN) tracking
  - Streaming API: `stream_changes(from_lsn) -> Stream<ChangeEntry>`
  - Batched change retrieval for efficiency
  - Checkpoint/resume support

- **Master-Slave Replication (Async)**
  - Primary mode: Accept writes, emit WAL
  - Replica mode: Apply WAL stream, reject writes
  - Async replication (eventually consistent)
  - Replication lag tracking
  - Automatic failover detection (future enhancement)

### CDC Entry Structure
```rust
pub struct ChangeLogEntry {
    pub lsn: u64,
    pub timestamp: u64,
    pub entity_type: String,
    pub entity_id: [u8; 16],
    pub change_type: ChangeType,  // Insert, Update, Delete
    pub changed_fields: Vec<String>,
    pub before: Option<Vec<u8>>,  // Full previous record
    pub after: Option<Vec<u8>>,   // Full new record
    pub schema_version: u64,
}
```

### CLI Commands
```
.replication status           Show replication role and lag
.replication stream <lsn>     Stream changes from LSN
```

### Success Criteria
- All mutations logged to changelog with LSN
- `stream_changes(lsn)` returns ordered changes
- Replica applies changes correctly
- Writes rejected on replica
- PubSubManager receives all change events
- Replication lag tracked and exposed
- Changelog compaction works with retention

---

## Phase 14: ORM Adapters

**Core tech:** async-nng (transport)

### Deliverables
- Prisma adapter
- SQLAlchemy adapter
- Django ORM adapter
- Hibernate/JPA adapter

### Success Criteria
- Each adapter passes ORM compatibility test suite
- CRUD operations, relations, and transactions work correctly
- Schema migrations translate to ORMDB migration bundles

---

## Phase 15: Benchmarking & Tuning

**Core tech:** all

### Deliverables
- Benchmark suite covering:
  - Aggregate fetch (N+1 elimination)
  - Transactional writes
  - Schema migrations
  - Cache invalidation
  - Columnar analytics queries
  - Replication lag
- Performance baselines established
- Optimization passes as needed

### Success Criteria
- Meets acceptance criteria defined in `benchmarks.md`
- Bounded internal query count for graph fetches
- No write outages for grade A/B migrations
- Stable pagination across relation includes
- Aggregations complete efficiently via columnar storage

---

## Phase Dependencies

```
Phase 0 (Bootstrap)
    │
    ▼
Phase 1 (Storage) ──────────────────────────┐
    │                                       │
    ▼                                       │
Phase 2 (Catalog) ◄─────────────────────────┤
    │                                       │
    ▼                                       │
Phase 3 (Protocol) ◄────────────────────────┤
    │                                       │
    ├───────────────┬───────────────┐       │
    ▼               ▼               ▼       │
Phase 4         Phase 5         Phase 6     │
(Query)         (Transport)     (Txn)       │
    │               │               │       │
    └───────────────┴───────────────┘       │
                    │                       │
                    ▼                       │
              Phase 7 (Migrations) ◄────────┘
                    │
                    ▼
              Phase 8 (Security)
                    │
                    ▼
              Phase 9 (Query Language)
                    │
                    ▼
              Phase 10 (CLI Tool)
                    │
                    ▼
              Phase 11 (Observability)
                    │
                    ▼
              Phase 12 (Hybrid Storage & Compaction)
                    │
                    ▼
              Phase 13 (CDC & Replication)
                    │
                    ▼
              Phase 14 (ORM Adapters)
                    │
                    ▼
              Phase 15 (Benchmarks)
```

## Current Status

**Completed phases:** Phase 0 (Project Bootstrap), Phase 1 (Storage Foundation), Phase 2 (Semantic Catalog), Phase 3 (Protocol & Serialization), Phase 4 (Query Engine), Phase 5 (Transport Layer), Phase 6 (Transactions & Constraints), Phase 7 (Migration Engine), Phase 8 (Security & Policies), Phase 9 (Query Language), Phase 10 (CLI Tool), Phase 11 (Observability)

**Active phase:** Phase 12 - Hybrid Storage & Compaction

**Next step:** Implement RetentionPolicy configuration and CompactionEngine
