# Migrations

ORMDB treats migrations as a first-class subsystem. Schema changes are declared as model bundles and compiled into online-safe migration plans whenever possible.

## Schema bundles

A schema bundle is a complete model graph definition:

- Entities, fields, relations.
- Constraints and policies.
- Defaults and computed fields.

Bundles are versioned and stored in the catalog.

## Diffing and planning

On update, ORMDB computes a structural diff between the current bundle and the new bundle. The migration planner produces:

- A sequence of steps.
- A safety grade.
- Required compatibility windows.

## Safety grades

- A: Online, no blocking writes.
- B: Online with background backfill or index build.
- C: Requires brief write lock.
- D: Not supported online.

The planner refuses unsafe online changes unless explicitly forced.

## Expand/contract workflow

Schema changes should follow expand/contract patterns:

1. Expand: add nullable fields, new tables, or new relations.
2. Backfill: background jobs fill new data.
3. Contract: enforce constraints, drop deprecated fields.

ORMDB can orchestrate backfills internally and track progress.

## Compatibility windows

Clients and servers may run multiple schema versions during deployment. ORMDB supports:

- A declared compatibility window.
- Field or relation deprecation markers.
- Safe defaults for new fields during the window.

## Validation and rollback

- Each migration step is validated against constraints.
- Long-running steps expose progress and ETA.
- Rollbacks are supported where data semantics allow.

## Observability

Migration status is visible through:

- Progress counters and timing.
- Safety grade and risk notes.
- Impact estimates on reads and writes.
