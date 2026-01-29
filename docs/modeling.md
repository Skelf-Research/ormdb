# Modeling

ORMDB treats the model graph as the source of truth. Schemas are defined in a typed model language or compiled from ORM definitions into a canonical bundle.

## Entities

An entity is a typed record with a stable identity.

- Every entity has a primary identity field (e.g., `id`).
- Identity must be unique and immutable.
- Entities may define defaults, required fields, and computed fields.

## Fields

Fields support common scalar and structured types:

- Scalars: integer, float, decimal, boolean, string, bytes, timestamp.
- Structured: enum, embedded struct, optional, array (where supported).
- Computed: materialized or virtual fields derived from other fields.

Computed fields must declare whether they are persisted or derived at read time.

## Relations

Relations are explicit and typed. The catalog records direction, cardinality, and delete behavior.

- One-to-one: unique foreign key.
- One-to-many: foreign key on the many side.
- Many-to-many: edge entity (join table).

Edge entities are first-class and can carry their own fields.

## Constraints and invariants

Constraints are declared at the model level and enforced at the storage level:

- Uniqueness constraints (single or composite).
- Foreign key constraints.
- Check constraints (e.g., `amount > 0`).
- Derived invariants (e.g., totals) if supported.

ORMDB rejects writes that violate constraints, even if the client adapter is buggy.

## Lifecycle rules

Entities can declare lifecycle rules such as:

- Cascading deletes or restrict deletes.
- Soft delete semantics with retention policies.
- Default ordering for stable pagination.

## Versioning

Each entity row can carry a version token for cache correctness and optimistic concurrency.

- Version tokens are monotonic per row.
- Updates return the new version token.
- Change streams emit versioned deltas for entities and relations.

## Policies

Policies are attached to entities and fields, not to raw tables:

- Row-level rules: which principals can read, write, or delete a row.
- Field-level rules: sensitive fields are only visible to authorized capabilities.
- Query shape limits: cap depth, fanout, or fields for a role.

Policies are compiled into execution rules and enforced server-side.
