# Schema Migrations

Safe schema evolution strategies for ORMDB.

## Overview

ORMDB supports schema migrations that allow you to evolve your data model over time without losing data or requiring downtime.

## Migration Types

### Safe Migrations (Non-Breaking)

These migrations are automatically applied without data loss:

| Change | Description |
|--------|-------------|
| Add entity | New entity type |
| Add field (nullable) | New optional field |
| Add field (with default) | New field with default value |
| Add relation | New relationship |
| Add index | New index on existing field |
| Widen type | int32 → int64, float32 → float64 |

### Breaking Migrations

These require explicit confirmation:

| Change | Description | Risk |
|--------|-------------|------|
| Remove entity | Delete entity type | Data loss |
| Remove field | Delete field | Data loss |
| Rename entity | Change entity name | Client updates needed |
| Rename field | Change field name | Client updates needed |
| Change type | Incompatible type change | Data conversion |
| Add required field | New non-null field without default | Existing rows fail |

## Creating Migrations

### Using the CLI

```bash
# Create a new migration
ormdb migrate create add_user_profile

# Creates: migrations/20240115120000_add_user_profile.json
```

### Migration File Structure

```json
{
  "version": "20240115120000",
  "name": "add_user_profile",
  "changes": [
    {
      "type": "add_entity",
      "entity": {
        "name": "Profile",
        "fields": [
          {"name": "id", "type": "uuid", "primary_key": true},
          {"name": "user_id", "type": "uuid"},
          {"name": "bio", "type": "string", "nullable": true},
          {"name": "avatar_url", "type": "string", "nullable": true}
        ]
      }
    },
    {
      "type": "add_relation",
      "relation": {
        "name": "profile",
        "from": "User",
        "to": "Profile",
        "cardinality": "one_to_one"
      }
    }
  ]
}
```

## Migration Operations

### Add Entity

```json
{
  "type": "add_entity",
  "entity": {
    "name": "Comment",
    "fields": [
      {"name": "id", "type": "uuid", "primary_key": true},
      {"name": "content", "type": "string"},
      {"name": "post_id", "type": "uuid"},
      {"name": "author_id", "type": "uuid"},
      {"name": "created_at", "type": "timestamp", "default": "now()"}
    ]
  }
}
```

### Add Field

```json
{
  "type": "add_field",
  "entity": "User",
  "field": {
    "name": "phone",
    "type": "string",
    "nullable": true
  }
}
```

### Add Field with Default

```json
{
  "type": "add_field",
  "entity": "User",
  "field": {
    "name": "status",
    "type": "string",
    "default": "active"
  }
}
```

### Remove Field

```json
{
  "type": "remove_field",
  "entity": "User",
  "field": "deprecated_field"
}
```

### Rename Field

```json
{
  "type": "rename_field",
  "entity": "User",
  "old_name": "name",
  "new_name": "full_name"
}
```

### Add Index

```json
{
  "type": "add_index",
  "entity": "User",
  "index": {
    "name": "user_email_idx",
    "fields": ["email"],
    "unique": true
  }
}
```

### Add Relation

```json
{
  "type": "add_relation",
  "relation": {
    "name": "posts",
    "from": "User",
    "to": "Post",
    "cardinality": "one_to_many",
    "foreign_key": "author_id"
  }
}
```

## Running Migrations

### Preview Changes

```bash
# Show pending migrations
ormdb migrate status

# Preview what will change
ormdb migrate run --dry-run
```

### Apply Migrations

```bash
# Apply all pending migrations
ormdb migrate run

# Apply specific number of migrations
ormdb migrate run --step 1

# Apply up to specific migration
ormdb migrate run --target 20240115120000
```

### Rollback Migrations

```bash
# Rollback last migration
ormdb migrate rollback

# Rollback multiple
ormdb migrate rollback --step 3

# Rollback to specific version
ormdb migrate rollback --target 20240101000000
```

## Programmatic Migrations

### Rust

```rust
use ormdb_core::migration::{Migration, MigrationChange};

let migration = Migration::new("add_user_avatar")
    .add_field("User", FieldDef::new("avatar_url", FieldType::String).nullable())
    .add_index("User", IndexDef::new("user_avatar_idx", vec!["avatar_url"]));

client.apply_migration(migration).await?;
```

### TypeScript

```typescript
import { Migration } from "@ormdb/client";

const migration = new Migration("add_user_avatar")
  .addField("User", {
    name: "avatar_url",
    type: "string",
    nullable: true,
  })
  .addIndex("User", {
    name: "user_avatar_idx",
    fields: ["avatar_url"],
  });

await client.applyMigration(migration);
```

### Python

```python
from ormdb import Migration

migration = Migration("add_user_avatar")
migration.add_field("User", {
    "name": "avatar_url",
    "type": "string",
    "nullable": True,
})
migration.add_index("User", {
    "name": "user_avatar_idx",
    "fields": ["avatar_url"],
})

client.apply_migration(migration)
```

## Data Migrations

For complex changes requiring data transformation:

```json
{
  "version": "20240115130000",
  "name": "split_name_field",
  "changes": [
    {
      "type": "add_field",
      "entity": "User",
      "field": {"name": "first_name", "type": "string", "nullable": true}
    },
    {
      "type": "add_field",
      "entity": "User",
      "field": {"name": "last_name", "type": "string", "nullable": true}
    },
    {
      "type": "data_migration",
      "script": "split_names.py"
    },
    {
      "type": "remove_field",
      "entity": "User",
      "field": "name"
    }
  ]
}
```

**split_names.py:**
```python
def migrate(client):
    users = client.query("User", fields=["id", "name"])
    for user in users.entities:
        parts = user["name"].split(" ", 1)
        first_name = parts[0]
        last_name = parts[1] if len(parts) > 1 else ""
        client.update("User", user["id"], {
            "first_name": first_name,
            "last_name": last_name,
        })
```

## Best Practices

### 1. Always Test Migrations

```bash
# Test on a copy of production data
ormdb backup create prod-backup.ormdb
ormdb backup restore prod-backup.ormdb --target ./test-data
ormdb migrate run --data-dir ./test-data --dry-run
```

### 2. Use Backward-Compatible Changes

Instead of renaming a field immediately:

```json
// Step 1: Add new field
{"type": "add_field", "entity": "User", "field": {"name": "full_name", "type": "string", "nullable": true}}

// Step 2: Deploy code that writes to both fields
// Step 3: Backfill data
// Step 4: Deploy code that reads from new field
// Step 5: Remove old field
{"type": "remove_field", "entity": "User", "field": "name"}
```

### 3. Keep Migrations Small

Split large changes into multiple migrations:

```bash
migrations/
├── 20240115120000_add_profile_entity.json
├── 20240115120001_add_profile_fields.json
├── 20240115120002_add_profile_relation.json
└── 20240115120003_add_profile_indexes.json
```

### 4. Include Rollback Logic

```json
{
  "version": "20240115120000",
  "name": "add_status_field",
  "changes": [...],
  "rollback": [
    {"type": "remove_field", "entity": "User", "field": "status"}
  ]
}
```

### 5. Version Control Migrations

```bash
# Commit migrations with your code
git add migrations/
git commit -m "Add user status field migration"
```

## Migration History

ORMDB tracks applied migrations:

```bash
# View migration history
ormdb migrate status

# Output:
# Applied migrations:
#   [x] 20240101120000_initial (applied 2024-01-01 12:00:00)
#   [x] 20240110150000_add_posts (applied 2024-01-10 15:00:00)
#   [ ] 20240115120000_add_status (pending)
```

Query migration history programmatically:

```rust
let history = client.migration_history().await?;
for migration in history {
    println!("{}: {} ({})",
        migration.version,
        migration.name,
        migration.applied_at);
}
```

## Troubleshooting

### Migration Failed Mid-Way

```bash
# Check status
ormdb migrate status

# If partially applied, fix data manually then mark as applied
ormdb migrate mark-applied 20240115120000
```

### Conflicting Migrations

When multiple developers create migrations:

```bash
# Rebase your migration
ormdb migrate rebase 20240115120000

# Or merge manually and update version number
```

### Verify Data Integrity

```bash
# After migration, verify data
ormdb admin verify

# Check specific entity
ormdb admin verify --entity User
```

---

## Next Steps

- **[Migration Safety Grades](../concepts/migration-safety.md)** - Understand A/B/C/D grading
- **[Security Guide](security.md)** - Implement access control
- **[Performance Guide](performance.md)** - Optimize your schema
