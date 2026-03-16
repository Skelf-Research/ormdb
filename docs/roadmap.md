# Roadmap

This document defines the implementation roadmap for ORMDB. Each phase includes deliverables, success criteria, and mapping to core technologies.

## Core Technologies

| Crate | Role |
|-------|------|
| **sled** | Embedded storage engine |
| **rkyv** | Zero-copy serialization |
| **mimalloc** | Global memory allocator |
| **async-nng** | Async messaging transport |

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

## Phase 9: Observability

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

## Phase 10: ORM Adapters

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

## Phase 11: Benchmarking & Tuning

**Core tech:** all

### Deliverables
- Benchmark suite covering:
  - Aggregate fetch (N+1 elimination)
  - Transactional writes
  - Schema migrations
  - Cache invalidation
- Performance baselines established
- Optimization passes as needed

### Success Criteria
- Meets acceptance criteria defined in `benchmarks.md`
- Bounded internal query count for graph fetches
- No write outages for grade A/B migrations
- Stable pagination across relation includes

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
              Phase 9 (Observability)
                    │
                    ▼
              Phase 10 (Adapters)
                    │
                    ▼
              Phase 11 (Benchmarks)
```

## Current Status

**Completed phases:** Phase 0 (Project Bootstrap), Phase 1 (Storage Foundation), Phase 2 (Semantic Catalog), Phase 3 (Protocol & Serialization), Phase 4 (Query Engine)

**Active phase:** None

**Next step:** Begin Phase 5 - Transport Layer
