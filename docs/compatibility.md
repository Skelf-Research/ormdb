# Compatibility and ORM adapters

ORMDB targets ORM workloads by integrating at the adapter level. The goal is to keep ORM APIs unchanged while replacing the underlying driver with an ORMDB adapter.

## Drop-in scope

"Drop-in" means:

- Models and query APIs remain unchanged.
- The ORM adapter translates ORM query structures into ORMDB IR.
- Transactions and migrations run through the adapter.

It does not mean full SQL compatibility or support for arbitrary raw SQL.

## Adapter responsibilities

An ORM adapter must:

- Map ORM query AST into graph fetch IR.
- Map results into ORM object graphs.
- Enforce ORM-level expectations for ordering and pagination.
- Bridge migration commands into schema bundle updates.
- Negotiate protocol versions with the server.

## Supported features (baseline)

- CRUD operations.
- Filters, ordering, and pagination.
- Relations and eager loading.
- Transactions.
- Uniqueness and foreign key constraints.
- Migrations driven by ORM schema definitions.

## Non-supported or limited features

- Raw SQL passthrough.
- Vendor-specific SQL extensions.
- Stored procedures or triggers defined in SQL.
- User-defined functions that depend on SQL execution.

These can be supported later or through explicit escape hatches, but are not required for initial compatibility.

## ORM-specific notes

- Prisma: adapter likely integrates at the query engine layer.
- SQLAlchemy ORM: adapter should intercept expression trees before SQL compilation.
- Django ORM: adapter should hook into the QuerySet compiler stage.
- Hibernate/JPA: adapter should intercept HQL or Criteria AST.

Each adapter must track ORM version changes because internal ASTs evolve.
