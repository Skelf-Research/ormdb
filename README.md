# ORMDB

ORMDB is an ORM-first relational database concept. The core idea is to make the model graph (entities, relations, invariants) the primary interface, while keeping a relational storage engine with real constraints, joins, and transactions.

This repository contains design documents for the system, intended to be detailed, precise, and usable as a foundation for an implementation plan.

## Goals

- Make ORM semantics first-class in the database kernel.
- Execute graph-shaped fetches without N+1 query patterns.
- Preserve relational guarantees: constraints, joins, and ACID transactions.
- Provide predictable output shapes for ORM clients.
- Make migrations safe and routine with built-in online workflows.
- Enable cache correctness with identity and change tracking primitives.

## Non-goals

- Full SQL compatibility as a primary interface.
- A document-store or schemaless database.
- A general-purpose GraphQL server.
- Supporting every vendor-specific SQL extension.
- Replacing specialized OLAP systems or warehouses (analytics mode is limited).

## Key concepts

- Model graph: entities and relations defined as typed schemas.
- Graph fetch: a query unit that declares root entity, filters, and include graph.
- Explicit relations: one-to-one, one-to-many, and many-to-many via edges.
- Compiled query protocol: typed query IR instead of ad-hoc SQL strings.
- Semantic catalog: database knows the entity graph and invariants.
- Migration engine: schema bundles, diffing, and online-safe plans.
- Identity and change streams: version tokens and entity/edge deltas.

## Status

Design phase. This repo captures the requirements, tradeoffs, and proposed architecture. There is no runtime implementation here yet.

## Documentation

- `docs/roadmap.md` - Implementation phases, milestones, and success criteria.
- `docs/technology.md` - Implementation language and core dependencies.
- `docs/architecture.md` - System layers, storage engine, and protocol.
- `docs/modeling.md` - Schema model, relations, constraints, and policies.
- `docs/querying.md` - Graph fetch semantics, output shape, and pagination.
- `docs/migrations.md` - Schema ingestion and online-safe migration rules.
- `docs/security.md` - Capabilities, RLS, field-level controls, auditing.
- `docs/compatibility.md` - ORM adapter strategy and drop-in scope.
- `docs/benchmarks.md` - Benchmark methodology and acceptance criteria.

## What success looks like

- ORM workloads execute with bounded query counts per request.
- Schema changes are online by default with clear safety grading.
- Cache invalidation is correct by construction via identity versioning.
- Observability makes plans, costs, and shape decisions explainable.

## License

TBD.
