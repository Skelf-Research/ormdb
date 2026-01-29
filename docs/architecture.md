# Architecture

ORMDB is a relational database with an ORM-first interface. The system is organized into layers that keep relational storage guarantees while exposing a typed, graph-oriented query surface.

## Layered overview

1. Storage engine
   - Row-oriented or hybrid row/column storage.
   - MVCC for transactional consistency.
   - Secondary indexes for filters and joins.
   - WAL for durability and replication.

2. Semantic catalog
   - Entity catalog: types, fields, and identities.
   - Relation catalog: cardinality and edge semantics.
   - Constraint catalog: uniqueness, checks, and foreign keys.
   - Policy catalog: row-level and field-level rules.
   - Schema history: versioned model bundles and migrations.

3. Query engine
   - Graph fetch compiler: translates model-level queries into relational plans.
   - Cost model: considers filters, includes, and output shape.
   - Plan cache: keyed by query IR signature.
   - Execution runtime: join strategies, batching, and result assembly.

4. Protocol
   - Typed query IR + parameters.
   - Typed results: entity blocks and edge blocks.
   - Change streams: entity and relation deltas.

5. Observability
   - Explain for graph fetches and relational plans.
   - Metrics: internal query count, fanout budgets, plan cache hit rate.
   - Structured audit logs at entity and field granularity.

## Storage engine assumptions

The storage engine is relational at its core. Constraints are enforced at write time. Joins are first-class and queryable. The design allows optional columnar projections for analytical scans, but the primary correctness model is OLTP.

## Execution paths

ORMDB supports two main execution modes:

- Graph fetch mode
  - Optimized for ORM workloads.
  - Returns structured entity and edge blocks that clients assemble into nested results.
  - Enforces fanout budgets and root-level pagination semantics.

- Relational mode (optional)
  - Returns flat, columnar or row-based results.
  - Intended for analytical or reporting workloads.

Both modes share the same catalog, constraints, and transaction model.

## Result formats

Graph fetch results are structured to allow clients to reconstruct ORM objects without relying on ad-hoc SQL joins:

- Entity blocks: columns for a single entity type, keyed by identity.
- Edge blocks: relation rows connecting entity identities.
- Optional ordering metadata for stable reconstruction.

Relational results are row or column batches with standard ordering rules.

## Protocol compatibility

The protocol version is part of the IR signature. The server advertises supported protocol versions and compatibility windows. Clients must negotiate a version and may be required to recompile IR on upgrade.

## Failure modes and guardrails

- Fanout limits prevent graph explosions.
- Query cost budgets cap total rows and edges returned.
- Policy enforcement is embedded in execution, not in client adapters.
- Migration engine emits safety grades and refuses unsafe online changes.

## Deployment model

- Single-node first, with optional read replicas.
- WAL-based replication for durability and change streams.
- Schema and policy metadata replicated as part of the catalog.
