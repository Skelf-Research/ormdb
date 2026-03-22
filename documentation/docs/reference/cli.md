# CLI Reference

Complete reference for the ORMDB command-line interface.

## Installation

```bash
# From cargo
cargo install ormdb-cli

# From package manager
brew install ormdb       # macOS
apt install ormdb        # Debian/Ubuntu
```

## Global Options

| Option | Short | Description |
|--------|-------|-------------|
| `--config` | `-c` | Configuration file path |
| `--host` | `-h` | Server host |
| `--port` | `-p` | Server port |
| `--verbose` | `-v` | Verbose output |
| `--quiet` | `-q` | Suppress non-error output |
| `--format` | `-f` | Output format (table, json, csv) |

---

## Server Commands

### ormdb server start

Start the ORMDB server.

```bash
ormdb server start [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--config` | `ormdb.toml` | Configuration file |
| `--data-dir` | `./data` | Data directory |
| `--port` | `8080` | Listen port |
| `--daemon` | false | Run as daemon |

**Examples:**

```bash
# Start with defaults
ormdb server start

# Start with custom config
ormdb server start --config /etc/ormdb/ormdb.toml

# Start as daemon
ormdb server start --daemon --data-dir /var/lib/ormdb
```

### ormdb server stop

Stop a running server.

```bash
ormdb server stop [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--pid-file` | PID file location |
| `--force` | Force shutdown |
| `--timeout` | Graceful shutdown timeout (seconds) |

### ormdb server status

Show server status.

```bash
ormdb server status [OPTIONS]
```

**Output:**
```
Server Status: Running
  PID: 12345
  Uptime: 2d 5h 30m
  Connections: 42
  Memory: 1.2 GB
  Data Size: 5.4 GB
```

---

## Schema Commands

### ormdb schema init

Initialize a new schema.

```bash
ormdb schema init [OPTIONS] <name>
```

**Examples:**

```bash
# Create new schema
ormdb schema init myapp

# Create with directory
ormdb schema init myapp --dir ./schemas
```

### ormdb schema validate

Validate a schema file.

```bash
ormdb schema validate <schema-file>
```

**Examples:**

```bash
# Validate schema
ormdb schema validate schema.json

# Validate with verbose output
ormdb schema validate schema.json -v
```

### ormdb schema apply

Apply a schema to the database.

```bash
ormdb schema apply [OPTIONS] <schema-file>
```

| Option | Description |
|--------|-------------|
| `--dry-run` | Show changes without applying |
| `--force` | Apply breaking changes |
| `--backup` | Create backup before applying |

**Examples:**

```bash
# Preview changes
ormdb schema apply schema.json --dry-run

# Apply schema
ormdb schema apply schema.json

# Apply with backup
ormdb schema apply schema.json --backup
```

### ormdb schema diff

Show differences between schemas.

```bash
ormdb schema diff <schema1> <schema2>
```

**Examples:**

```bash
# Compare schema files
ormdb schema diff old-schema.json new-schema.json

# Compare file with running server
ormdb schema diff new-schema.json --server localhost:8080
```

### ormdb schema export

Export current schema.

```bash
ormdb schema export [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--output` | Output file (stdout if not set) |
| `--format` | Format (json, yaml, rust) |

**Examples:**

```bash
# Export to file
ormdb schema export --output schema.json

# Export as YAML
ormdb schema export --format yaml
```

---

## Query Commands

### ormdb query

Execute a query.

```bash
ormdb query [OPTIONS] <entity> [FILTER]
```

| Option | Description |
|--------|-------------|
| `--fields` | Fields to select |
| `--include` | Relations to include |
| `--order` | Order by field |
| `--limit` | Limit results |
| `--offset` | Offset for pagination |

**Examples:**

```bash
# Basic query
ormdb query User

# With fields and filter
ormdb query User --fields id,name,email "status = 'active'"

# With relations
ormdb query User --include posts --limit 10

# Complex filter
ormdb query User "age >= 18 AND status = 'active'" --order name

# Output as JSON
ormdb query User --format json
```

### ormdb aggregate

Execute an aggregate query.

```bash
ormdb aggregate [OPTIONS] <entity> <aggregations>
```

**Examples:**

```bash
# Count users
ormdb aggregate User count

# Multiple aggregations
ormdb aggregate Order "count,sum(total),avg(total)"

# With filter
ormdb aggregate Order "sum(total)" "status = 'completed'"
```

---

## Mutation Commands

### ormdb insert

Insert entities.

```bash
ormdb insert [OPTIONS] <entity> <data>
```

**Examples:**

```bash
# Insert single entity
ormdb insert User '{"name": "Alice", "email": "alice@example.com"}'

# Insert from file
ormdb insert User --file users.json

# Insert multiple
ormdb insert User --file users.ndjson --format ndjson
```

### ormdb update

Update entities.

```bash
ormdb update [OPTIONS] <entity> <data> [FILTER]
```

**Examples:**

```bash
# Update by ID
ormdb update User '{"status": "inactive"}' --id 550e8400-...

# Update by filter
ormdb update User '{"verified": true}' "email LIKE '%@company.com'"
```

### ormdb delete

Delete entities.

```bash
ormdb delete [OPTIONS] <entity> [FILTER]
```

| Option | Description |
|--------|-------------|
| `--id` | Delete by ID |
| `--cascade` | Cascade delete |
| `--dry-run` | Show what would be deleted |

**Examples:**

```bash
# Delete by ID
ormdb delete User --id 550e8400-...

# Delete by filter
ormdb delete Session "expires_at < now()"

# Cascade delete
ormdb delete User --id 550e8400-... --cascade

# Preview delete
ormdb delete User "status = 'inactive'" --dry-run
```

---

## Migration Commands

### ormdb migrate create

Create a new migration.

```bash
ormdb migrate create [OPTIONS] <name>
```

**Examples:**

```bash
# Create migration
ormdb migrate create add_user_avatar

# Creates: migrations/20240115120000_add_user_avatar.json
```

### ormdb migrate status

Show migration status.

```bash
ormdb migrate status
```

**Output:**
```
Migrations:
  [x] 20240101120000_initial
  [x] 20240110150000_add_posts
  [ ] 20240115120000_add_user_avatar
```

### ormdb migrate run

Run pending migrations.

```bash
ormdb migrate run [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--dry-run` | Preview without applying |
| `--step` | Run only N migrations |
| `--target` | Run up to specific migration |

### ormdb migrate rollback

Rollback migrations.

```bash
ormdb migrate rollback [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--step` | Rollback N migrations |
| `--target` | Rollback to specific migration |

---

## Backup Commands

### ormdb backup create

Create a backup.

```bash
ormdb backup create [OPTIONS] <output>
```

| Option | Description |
|--------|-------------|
| `--compress` | Compress backup |
| `--encrypt` | Encrypt backup |
| `--parallel` | Parallel backup threads |

**Examples:**

```bash
# Create backup
ormdb backup create backup.ormdb

# Compressed backup
ormdb backup create backup.ormdb.gz --compress

# To S3
ormdb backup create s3://bucket/backups/backup.ormdb
```

### ormdb backup restore

Restore from backup.

```bash
ormdb backup restore [OPTIONS] <input>
```

| Option | Description |
|--------|-------------|
| `--target` | Target data directory |
| `--verify` | Verify after restore |

### ormdb backup list

List available backups.

```bash
ormdb backup list [OPTIONS]
```

---

## Admin Commands

### ormdb admin stats

Show database statistics.

```bash
ormdb admin stats [OPTIONS]
```

**Output:**
```
Database Statistics:
  Entities:
    User: 10,542
    Post: 45,231
    Comment: 123,456

  Storage:
    Data size: 2.3 GB
    Index size: 456 MB
    WAL size: 128 MB

  Performance:
    Cache hit rate: 94.2%
    Avg query time: 2.3ms
```

### ormdb admin compact

Compact database storage.

```bash
ormdb admin compact [OPTIONS]
```

### ormdb admin reindex

Rebuild indexes.

```bash
ormdb admin reindex [OPTIONS] [ENTITY]
```

**Examples:**

```bash
# Reindex all
ormdb admin reindex

# Reindex specific entity
ormdb admin reindex User
```

### ormdb admin vacuum

Reclaim storage space.

```bash
ormdb admin vacuum [OPTIONS]
```

---

## Config Commands

### ormdb config validate

Validate configuration file.

```bash
ormdb config validate [OPTIONS] <config-file>
```

### ormdb config show

Show effective configuration.

```bash
ormdb config show [OPTIONS]
```

---

## Interactive Shell

### ormdb shell

Start interactive shell.

```bash
ormdb shell [OPTIONS]
```

**Shell Commands:**

```
ormdb> .help              # Show help
ormdb> .tables            # List entities
ormdb> .schema User       # Show entity schema
ormdb> .exit              # Exit shell

ormdb> SELECT * FROM User WHERE status = 'active' LIMIT 10;
ormdb> INSERT INTO User (name, email) VALUES ('Alice', 'alice@example.com');
```

---

## Output Formats

| Format | Description |
|--------|-------------|
| `table` | Human-readable table (default) |
| `json` | JSON output |
| `jsonl` | JSON Lines (one object per line) |
| `csv` | Comma-separated values |
| `yaml` | YAML output |

**Examples:**

```bash
# Table output (default)
ormdb query User --limit 5

# JSON output
ormdb query User --limit 5 --format json

# CSV for export
ormdb query User --format csv > users.csv
```

---

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Connection error |
| 4 | Query error |
| 5 | Schema error |
| 6 | Permission denied |

---

## Next Steps

- **[Configuration Reference](configuration.md)** - All configuration options
- **[Deployment Guide](../operations/deployment.md)** - Deploy ORMDB
- **[Backup & Restore](../operations/backup-restore.md)** - Data protection commands
