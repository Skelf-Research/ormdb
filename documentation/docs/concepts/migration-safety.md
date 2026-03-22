# Migration Safety

ORMDB analyzes schema changes and assigns safety grades (A/B/C/D) based on their impact. This helps you understand the risk of each change and plan deployments accordingly.

---

## The Problem with Migrations

Schema migrations in traditional databases are error-prone:

- **Silent data loss**: Dropping a column destroys data
- **Downtime**: Adding a NOT NULL column locks the table
- **Breaking changes**: Renaming a field breaks clients
- **Rollback difficulty**: Some changes can't be undone

ORMDB's migration system makes these risks explicit and provides guardrails.

---

## Safety Grades

Every schema change is assigned a safety grade:

| Grade | Impact | Online? | Example |
|-------|--------|---------|---------|
| **A** | Non-breaking | Yes | Add optional field |
| **B** | Background work | Yes | Add index |
| **C** | Brief disruption | Partial | Narrow type |
| **D** | Destructive | No | Remove field |

### Grade A: Safe Changes

These changes have no impact on existing data or clients.

**Examples:**
- Add a new entity
- Add an optional field
- Add a new relation
- Remove an index
- Change default value
- Add enum variants

```rust
// Grade A: Add optional field
let from = SchemaBundle::new(1)
    .with_entity(user_entity());

let mut user_with_bio = user_entity();
user_with_bio.fields.push(
    FieldDef::optional("bio", FieldType::Scalar(ScalarType::String))
);

let to = SchemaBundle::new(2)
    .with_entity(user_with_bio);

let grade = SafetyGrader::grade(&SchemaDiff::compute(&from, &to));
assert_eq!(grade.overall_grade, SafetyGrade::A);
```

### Grade B: Online with Backfill

These changes can be performed online but require background work.

**Examples:**
- Add required field with default value
- Add an index
- Widen numeric type (Int32 → Int64)
- Add unique constraint (requires validation)
- Add foreign key (requires validation)

```rust
// Grade B: Add required field with default
let mut user_with_status = user_entity();
user_with_status.fields.push(
    FieldDef::new("status", FieldType::Scalar(ScalarType::String))
        .with_default(DefaultValue::String("active".into()))
);

// Requires backfill to populate default for existing rows
```

### Grade C: Requires Care

These changes may cause brief disruption or require careful handling.

**Examples:**
- Narrow numeric type (Int64 → Int32) - may lose data
- Make optional field required (without default)
- Disable soft delete

```rust
// Grade C: Narrow type
// From: age Int64
// To: age Int32

let grade = SafetyGrader::grade(&diff);
assert_eq!(grade.overall_grade, SafetyGrade::C);
assert!(!grade.can_run_online());
```

### Grade D: Destructive

These changes destroy data or break compatibility. They require explicit confirmation.

**Examples:**
- Remove an entity
- Remove a field
- Remove enum variants
- Change identity field
- Add required field without default
- Change relation cardinality

```rust
// Grade D: Remove field
let from = SchemaBundle::new(1)
    .with_entity(user_with_email());

let to = SchemaBundle::new(2)
    .with_entity(user_without_email());  // email field removed

let grade = SafetyGrader::grade(&SchemaDiff::compute(&from, &to));
assert_eq!(grade.overall_grade, SafetyGrade::D);
assert!(grade.requires_data_migration());
```

---

## Grading Rules

### Entity Changes

| Change | Grade | Reasoning |
|--------|-------|-----------|
| Add entity | A | New entities don't affect existing data |
| Remove entity | D | Destroys all entity data |
| Change identity field | D | Breaks referential integrity |

### Field Changes

| Change | Grade | Reasoning |
|--------|-------|-----------|
| Add optional field | A | Existing data unaffected |
| Add required with default | B | Needs backfill |
| Add required without default | D | Impossible for existing rows |
| Remove field | D | Destroys data |
| Make required → optional | A | Relaxes constraint |
| Make optional → required (with default) | B | Needs backfill |
| Make optional → required (no default) | D | May have NULL values |
| Change default | A | Only affects new records |
| Add index | B | Background index build |
| Remove index | A | Safe, just slower queries |

### Type Changes

| Change | Grade | Reasoning |
|--------|-------|-----------|
| Int32 → Int64 | B | Safe widening |
| Float32 → Float64 | B | Safe widening |
| Int64 → Int32 | C | May lose precision |
| String → any | D | Generally incompatible |
| Add enum variant | A | Existing values unaffected |
| Remove enum variant | D | Existing values may use it |

### Relation Changes

| Change | Grade | Reasoning |
|--------|-------|-----------|
| Add relation | A | Non-breaking |
| Remove relation | D | Breaks referential integrity |
| Change cardinality | D | May violate constraints |
| Change delete behavior | B | Only affects future deletes |

### Constraint Changes

| Change | Grade | Reasoning |
|--------|-------|-----------|
| Add unique constraint | B | Must validate existing data |
| Add foreign key | B | Must validate references |
| Add check constraint | B | Must validate existing data |
| Remove any constraint | A | Relaxes validation |

---

## Using the Grader

### Basic Usage

```rust
use ormdb_core::migration::{SchemaDiff, SafetyGrader};

let from = catalog.current_schema()?;
let to = new_schema;

let diff = SchemaDiff::compute(&from, &to);
let grade = SafetyGrader::grade(&diff);

println!("Overall grade: {:?}", grade.overall_grade);
println!("Can run online: {}", grade.can_run_online());
println!("Requires backfill: {}", grade.requires_backfill());
```

### Inspecting Change Grades

```rust
for change_grade in &grade.change_grades {
    println!("{}", change_grade.change_description);
    println!("  Grade: {:?}", change_grade.grade);
    println!("  Reason: {}", change_grade.reasoning);

    if change_grade.requires_backfill {
        println!("  Requires backfill");
    }
    if change_grade.requires_data_migration {
        println!("  Requires data migration");
    }
}
```

### Handling Blocking Changes

```rust
if !grade.blocking_changes.is_empty() {
    println!("The following changes require attention:");
    for change in &grade.blocking_changes {
        println!("  - {} (Grade {:?})", change.change_description, change.grade);
    }
}
```

### Warnings

The grader also produces warnings for risky patterns:

```rust
for warning in &grade.warnings {
    println!("Warning: {}", warning);
}

// Example warnings:
// - "Multiple destructive changes (3) detected - consider breaking into smaller migrations"
// - "Multiple backfill operations (5) may take significant time"
// - "Removing entity 'User' also removes 2 relation(s)"
```

---

## Migration Workflow

### 1. Design Migration

```rust
let new_schema = SchemaBundle::new(current_version + 1)
    .with_entity(/* ... */);
```

### 2. Grade Migration

```rust
let diff = SchemaDiff::compute(&current, &new_schema);
let grade = SafetyGrader::grade(&diff);
```

### 3. Review Grade

```rust
match grade.overall_grade {
    SafetyGrade::A => {
        println!("Safe to deploy immediately");
    }
    SafetyGrade::B => {
        println!("Safe to deploy, will run background tasks");
        estimate_backfill_time(&grade)?;
    }
    SafetyGrade::C => {
        println!("Review carefully before deploying");
        require_approval("migration-c-grade")?;
    }
    SafetyGrade::D => {
        println!("Destructive change - requires explicit confirmation");
        require_approval("migration-destructive")?;
        backup_affected_data(&diff)?;
    }
}
```

### 4. Apply Migration

```rust
if can_proceed {
    catalog.apply_schema(new_schema)?;

    if grade.requires_backfill() {
        migration_executor.run_backfills(&diff)?;
    }
}
```

---

## Safe Migration Patterns

### Adding a Required Field

**Wrong (Grade D):**
```rust
// Fails: existing rows have no value
FieldDef::new("status", FieldType::Scalar(ScalarType::String))
```

**Right (Grade B):**
```rust
// Works: default value used for backfill
FieldDef::new("status", FieldType::Scalar(ScalarType::String))
    .with_default(DefaultValue::String("active".into()))
```

### Renaming a Field

**Wrong (Grade D):**
```rust
// Remove old, add new = data loss
// Migration 1: Remove 'name'
// Migration 2: Add 'full_name'
```

**Right (Grade A + B):**
```rust
// Migration 1: Add new field (Grade A)
.with_field(FieldDef::optional("full_name", ...))

// Migration 2: Backfill new field from old (application code)
for user in users {
    user.full_name = user.name;
}

// Migration 3: Remove old field (Grade D, but data preserved)
// Only after all clients updated
```

### Changing Field Type

**Wrong (Grade D):**
```rust
// From: age String
// To: age Int32
// Data loss: can't convert "twenty-five" to int
```

**Right:**
```rust
// Migration 1: Add new field (Grade A)
.with_field(FieldDef::optional("age_int", Int32))

// Migration 2: Backfill with validation (application code)
for user in users {
    if let Ok(age) = user.age.parse::<i32>() {
        user.age_int = Some(age);
    }
}

// Migration 3: Make new field required (Grade B)
// Migration 4: Remove old field (Grade D)
```

### Adding an Index

```rust
// Grade B: Background index build
FieldDef::new("email", FieldType::Scalar(ScalarType::String))
    .with_index()

// The grader reports:
// - Grade: B
// - Reason: "Index build runs in background"
// - Requires backfill: true
```

---

## CLI Integration

The ORMDB CLI shows migration grades:

```bash
$ ormdb migrate preview

Schema Migration Preview
========================
From version: 5
To version: 6

Changes:
  [A] Add optional field 'User.bio'
  [B] Add index on 'User.email'
  [D] Remove field 'User.legacy_id'

Overall Grade: D (Destructive)

Warnings:
  - Removing 'User.legacy_id' will destroy data for 15,234 users

To apply this migration, confirm with:
  ormdb migrate apply --confirm-destructive
```

---

## Best Practices

### 1. Small, Incremental Migrations

```rust
// Bad: One big migration with many changes
let migration = big_schema_change();  // Grade D with 10 changes

// Good: Many small migrations
let m1 = add_optional_field();    // Grade A
let m2 = backfill_field();        // Application code
let m3 = make_field_required();   // Grade B
let m4 = remove_old_field();      // Grade D (after validation)
```

### 2. Test Migrations Before Production

```rust
#[test]
fn test_migration_grade() {
    let from = current_schema();
    let to = new_schema();
    let grade = SafetyGrader::grade(&SchemaDiff::compute(&from, &to));

    assert!(grade.can_run_online(), "Migration should be online-safe");
}
```

### 3. Document Grade D Changes

```rust
// Migration: Remove User.legacy_id
//
// Why: This field hasn't been used since v2.0 (6 months ago)
// Risk: 15K users have values, but analysis shows no reads in logs
// Backup: Full backup taken 2024-01-15
// Rollback: Restore from backup if issues found
```

### 4. Monitor Backfill Progress

```rust
let progress = migration_executor.backfill_progress()?;
println!("Backfill: {}/{} complete", progress.completed, progress.total);
println!("ETA: {}", progress.estimated_completion);
```

---

## Next Steps

- **[Schema Migrations Guide](../guides/schema-migrations.md)** - Step-by-step migration guide
- **[Schema Design Tutorial](../tutorials/schema-design.md)** - Designing schemas
- **[CLI Reference](../reference/cli.md)** - Migration CLI commands
