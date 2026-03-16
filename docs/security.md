# Security

ORMDB enforces security at the model level, not at the SQL string level. The database understands entities and relations and applies policies directly in execution.

## Threat model

ORMDB aims to reduce common risks in ORM-backed systems:

- Over-broad reads caused by missing filters.
- Unauthorized access due to adapter bugs.
- Data leakage via unsafe escape hatches or ad-hoc joins.
- Query-based denial of service via deep graphs.

It does not attempt to replace application-level authentication or identity management.

## Capabilities

Connections carry a set of capabilities that define allowed operations:

- Read or write permissions per entity.
- Optional field-level scopes.
- Explicit allowance for high-cost queries.

Capabilities are checked by the engine, not the client.

## Row-level security

Policies can restrict which rows are visible or mutable based on session context:

- Ownership rules (e.g., user can read own records).
- Team or role-based access.
- Multi-tenant isolation.

RLS is compiled into query execution so it applies to all query modes.

## Field-level security

Fields can be marked as sensitive. The engine will either:

- Omit them from results, or
- Mask them unless capability permits access.

## Query shape limits

To prevent graph-explosion attacks:

- Enforce maximum depth and fanout.
- Cap total rows and edges returned.
- Require explicit opt-in for expensive includes.

## Auditing

Operations are logged at the model level:

- Entity and field changes.
- Relation changes (edge inserts/deletes).
- Policy denials and overrides.

Audit logs are structured and queryable for compliance.
