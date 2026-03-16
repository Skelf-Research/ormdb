# Querying

ORMDB queries are typed and graph-oriented. The primary unit is a graph fetch that declares a root entity, filters, and an include graph. The database compiles this into one or more relational plans with bounded fanout.

## Graph fetch shape

A graph fetch specifies:

- Root entity type.
- Filter predicate on the root.
- Projection (fields to return per entity).
- Include graph (relations to load).
- Ordering and pagination for the root.
- Optional budgets for depth and fanout.

Example (conceptual):

```
GraphFetch {
  root: User
  filter: id = $1
  select: [id, name]
  include: {
    teams: {
      select: [id, name]
      include: {
        members: { select: [id, name] }
      }
    }
  }
}
```

## Output shape

Graph fetch results are structured in blocks rather than raw rows:

- Entity blocks: typed columns per entity type, keyed by identity.
- Edge blocks: pairs of identities representing relations.
- Ordering metadata for stable reconstruction.

Clients reconstruct nested objects from these blocks without ambiguous duplicate rows.

## Pagination semantics

Pagination applies to the root entity only. Includes are resolved for each root row returned.

Rules:

- Root ordering is required for pagination.
- Keyset pagination is the default for stability.
- Offset pagination may be allowed with warnings and cost limits.
- Includes do not change root ordering.
- Deduplication is deterministic and stable.

These rules avoid pagination drift caused by join multiplicity.

## Fanout and cost budgets

Graph fetches can explode in size. ORMDB enforces budgets:

- Max depth: limits include nesting.
- Max fanout per relation.
- Max total rows and edges returned.
- Per-query execution time budget.

Queries that exceed budgets are rejected or require explicit overrides.

## Relational queries (optional)

ORMDB can expose a secondary relational mode for analytics:

- Flat result sets.
- Columnar batches for vectorized execution.
- Explicit aggregates.

Relational mode still uses typed IR and shares the same catalog and policies. It is limited in scope and not intended to replace full OLAP systems.

## Explainability

Every query can be explained in terms of:

- Logical graph steps.
- Chosen join or batch strategy.
- Cost estimates and budget usage.
- Index or projection usage.
